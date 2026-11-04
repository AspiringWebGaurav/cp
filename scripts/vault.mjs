#!/usr/bin/env node

/**
 * CP — Encrypted Workspace Vault Pipeline
 * 
 * Securely archives, compresses, and encrypts all private workspace assets:
 * - Environment Credentials: `.env.local`
 * - Offline Brain & Project Rules: `.local/` (rules, project maps, user directives)
 * - Proprietary Automation Scripts: `scripts/ship.mjs`
 * - Architecture Manuals: `docs/`
 * - Release Signing Keys: `.tauri/` (if present)
 * 
 * Encryption: AES-256-GCM + scrypt key derivation (memory-hard, GPU-resistant).
 * Container: 8-byte magic ('CP__VALT') + version (1 byte) + salt (32 bytes) + IV (12 bytes) + tag (16 bytes) + ciphertext.
 * Master Default Password: '5737.5737'
 * 
 * Usage:
 *   node scripts/vault.mjs pack [--pass <password>]
 *   node scripts/vault.mjs unpack [--pass <password>]
 *   node scripts/vault.mjs status
 */

import fs from 'fs';
import path from 'path';
import crypto from 'crypto';
import zlib from 'zlib';
import readline from 'readline';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '..');
const vaultFilePath = path.join(rootDir, 'cp-vault.enc');

const MAGIC_HEADER = Buffer.from('CP__VALT', 'utf8'); // 8 bytes
const FORMAT_VERSION = 0x01; // 1 byte
const SALT_LEN = 32;
const IV_LEN = 12;
const TAG_LEN = 16;
const SCRYPT_N = 16384;
const SCRYPT_R = 8;
const SCRYPT_P = 1;

const DEFAULT_VAULT_PASS = '5737.5737';

const EXCLUDE_DIR_NAMES = new Set([
  'node_modules',
  'target',
  'dist',
  '.git',
  '.cache',
]);

// 1. Resolve Password (CLI arg > env > default '5737.5737')
async function resolvePassword(action) {
  for (let i = 2; i < process.argv.length; i++) {
    const arg = process.argv[i];
    if (arg === '--pass' || arg === '--password' || arg === '-p') {
      const val = process.argv[i + 1];
      if (val) return val;
    } else if (arg.startsWith('--pass=')) {
      return arg.split('=')[1];
    } else if (arg.startsWith('--password=')) {
      return arg.split('=')[1];
    }
  }

  if (process.env.CP_VAULT_PASS) return process.env.CP_VAULT_PASS;
  if (process.env.VAULT_PASSWORD) return process.env.VAULT_PASSWORD;

  // Use configured project default password
  return DEFAULT_VAULT_PASS;
}

// 2. Recursive File Collector
function collectFiles(baseDir, relativePrefix = '') {
  const results = [];
  if (!fs.existsSync(baseDir)) return results;

  const entries = fs.readdirSync(baseDir, { withFileTypes: true });
  for (const entry of entries) {
    const fullPath = path.join(baseDir, entry.name);
    const relPath = relativePrefix ? path.posix.join(relativePrefix, entry.name) : entry.name;

    if (entry.isDirectory()) {
      if (EXCLUDE_DIR_NAMES.has(entry.name)) continue;
      results.push(...collectFiles(fullPath, relPath));
    } else if (entry.isFile()) {
      if (entry.name === 'cp-vault.enc') continue;
      results.push({ fullPath, relPath });
    }
  }
  return results;
}

// 3. PACK Command: Collect -> JSON -> Gzip -> AES-256-GCM -> cp-vault.enc
async function packVault() {
  console.log('\n===============================================================');
  console.log('   🔒 CP — Packaging Encrypted Workspace Vault');
  console.log('===============================================================\n');

  const password = await resolvePassword('PACK');
  console.log(`[1/4] Scanning local offline brain, secrets, and configuration...`);
  const filesMap = new Map();

  function addFile(fullPath, relPath) {
    const normalizedRel = relPath.replace(/\\/g, '/');
    if (!filesMap.has(normalizedRel) && fs.existsSync(fullPath)) {
      filesMap.set(normalizedRel, { fullPath, relPath: normalizedRel });
    }
  }

  // A. Root environment secrets (.env.local, .env.* except .env.example)
  if (fs.existsSync(rootDir)) {
    const rootEntries = fs.readdirSync(rootDir, { withFileTypes: true });
    for (const entry of rootEntries) {
      if (entry.isFile()) {
        const name = entry.name.toLowerCase();
        if (name.startsWith('.env') && !name.includes('.example') && !name.includes('.sample')) {
          addFile(path.join(rootDir, entry.name), entry.name);
        }
      }
    }
  }

  // B. Offline Brain & Local Directives (.local/)
  const localDir = path.join(rootDir, '.local');
  if (fs.existsSync(localDir)) {
    for (const item of collectFiles(localDir, '.local')) {
      addFile(item.fullPath, item.relPath);
    }
  }

  // C. Documentation & Specifications (docs/)
  const docsDir = path.join(rootDir, 'docs');
  if (fs.existsSync(docsDir)) {
    for (const item of collectFiles(docsDir, 'docs')) {
      addFile(item.fullPath, item.relPath);
    }
  }

  // D. Proprietary Scripts (scripts/ship.mjs)
  const shipScript = path.join(rootDir, 'scripts', 'ship.mjs');
  if (fs.existsSync(shipScript)) {
    addFile(shipScript, 'scripts/ship.mjs');
  }

  // E. Tauri Signing Keys (.tauri/ if present)
  const tauriDir = path.join(rootDir, '.tauri');
  if (fs.existsSync(tauriDir)) {
    for (const item of collectFiles(tauriDir, '.tauri')) {
      addFile(item.fullPath, item.relPath);
    }
  }

  if (filesMap.size === 0) {
    console.error('❌ No sensitive files found to pack.');
    process.exit(1);
  }

  console.log(`[2/4] Bundling ${filesMap.size} sensitive files:`);
  const manifest = {
    version: '0.0.1',
    created: new Date().toISOString(),
    files: [],
  };

  let uncompressedBytes = 0;
  for (const [relPath, { fullPath }] of filesMap.entries()) {
    const content = fs.readFileSync(fullPath);
    uncompressedBytes += content.length;
    manifest.files.push({
      path: relPath,
      data: content.toString('base64'),
    });
    console.log(`  ✓ + ${relPath} (${content.length} bytes)`);
  }

  console.log(`[3/4] Compressing manifest (${(uncompressedBytes / 1024).toFixed(1)} KB uncompressed)...`);
  const jsonBuffer = Buffer.from(JSON.stringify(manifest), 'utf8');
  const compressedData = zlib.gzipSync(jsonBuffer, { level: 9 });

  console.log(`[4/4] Encrypting with AES-256-GCM + Scrypt (Memory-Hard)...`);
  const salt = crypto.randomBytes(SALT_LEN);
  const iv = crypto.randomBytes(IV_LEN);

  const key = crypto.scryptSync(password, salt, 32, {
    N: SCRYPT_N,
    r: SCRYPT_R,
    p: SCRYPT_P,
  });

  const cipher = crypto.createCipheriv('aes-256-gcm', key, iv);
  const ciphertext = Buffer.concat([cipher.update(compressedData), cipher.final()]);
  const tag = cipher.getAuthTag();

  // Container layout: MAGIC (8) + VERSION (1) + SALT (32) + IV (12) + TAG (16) + CIPHERTEXT
  const vaultBuffer = Buffer.concat([
    MAGIC_HEADER,
    Buffer.from([FORMAT_VERSION]),
    salt,
    iv,
    tag,
    ciphertext,
  ]);

  fs.writeFileSync(vaultFilePath, vaultBuffer);

  console.log('\n===============================================================');
  console.log(`  ✅ VAULT CREATED SUCCESSFULLY: ${path.basename(vaultFilePath)}`);
  console.log(`  📦 Encrypted Container Size: ${(vaultBuffer.length / 1024).toFixed(1)} KB`);
  console.log(`  🔑 Password: ${password === DEFAULT_VAULT_PASS ? '5737.5737 (Default Master)' : 'Custom'}`);
  console.log(`  ✨ When cloning after PC format, run: npm run vault:unpack`);
  console.log('===============================================================\n');
}

// 4. UNPACK Command: Read -> AES-256-GCM Decrypt -> Gunzip -> Restore on Disk
async function unpackVault() {
  console.log('\n===============================================================');
  console.log('   🔓 CP — Unpacking Encrypted Workspace Vault');
  console.log('===============================================================\n');

  if (!fs.existsSync(vaultFilePath)) {
    console.error(`❌ Vault file not found: ${vaultFilePath}`);
    console.error('Make sure cp-vault.enc is present in the repository root.');
    process.exit(1);
  }

  const password = await resolvePassword('UNPACK');
  const vaultBuffer = fs.readFileSync(vaultFilePath);

  if (vaultBuffer.length < 8 + 1 + SALT_LEN + IV_LEN + TAG_LEN) {
    console.error('❌ Corrupt vault container: file too small.');
    process.exit(1);
  }

  // Verify magic header
  const magic = vaultBuffer.subarray(0, 8);
  if (!magic.equals(MAGIC_HEADER)) {
    console.error('❌ Invalid container format: magic header mismatch.');
    process.exit(1);
  }

  let offset = 8;
  const version = vaultBuffer.readUInt8(offset);
  offset += 1;

  const salt = vaultBuffer.subarray(offset, offset + SALT_LEN);
  offset += SALT_LEN;

  const iv = vaultBuffer.subarray(offset, offset + IV_LEN);
  offset += IV_LEN;

  const tag = vaultBuffer.subarray(offset, offset + TAG_LEN);
  offset += TAG_LEN;

  const ciphertext = vaultBuffer.subarray(offset);

  console.log(`[1/3] Deriving key and authenticating AES-256-GCM ciphertext...`);
  const key = crypto.scryptSync(password, salt, 32, {
    N: SCRYPT_N,
    r: SCRYPT_R,
    p: SCRYPT_P,
  });

  let compressedData;
  try {
    const decipher = crypto.createDecipheriv('aes-256-gcm', key, iv);
    decipher.setAuthTag(tag);
    compressedData = Buffer.concat([decipher.update(ciphertext), decipher.final()]);
  } catch (err) {
    console.error('\n❌ DECRYPTION FAILED: Invalid passphrase or corrupted vault file.');
    process.exit(1);
  }

  console.log(`[2/3] Decompressing file manifest...`);
  const jsonBuffer = zlib.gunzipSync(compressedData);
  const manifest = JSON.parse(jsonBuffer.toString('utf8'));

  console.log(`[3/3] Restoring ${manifest.files.length} files to workspace:`);
  for (const item of manifest.files) {
    const fullPath = path.join(rootDir, item.path);
    const parentDir = path.dirname(fullPath);
    if (!fs.existsSync(parentDir)) {
      fs.mkdirSync(parentDir, { recursive: true });
    }
    const content = Buffer.from(item.data, 'base64');
    fs.writeFileSync(fullPath, content);
    console.log(`  ✓ Extracted: ${item.path} (${content.length} bytes)`);
  }

  console.log('\n===============================================================');
  console.log('  ✨ VAULT RESTORED SUCCESSFULLY!');
  console.log('  All private assets, .env.local, and .local/ brain restored.');
  console.log('  Workspace is 100% identical to pre-format state.');
  console.log('===============================================================\n');
}

// 5. STATUS Command
function statusVault() {
  console.log('\n===============================================================');
  console.log('   ℹ️ CP — Vault Status');
  console.log('===============================================================\n');

  if (fs.existsSync(vaultFilePath)) {
    const stat = fs.statSync(vaultFilePath);
    console.log(`  Status:    EXISTS`);
    console.log(`  Location:  ${vaultFilePath}`);
    console.log(`  File Size: ${(stat.size / 1024).toFixed(1)} KB`);
    console.log(`  Modified:  ${stat.mtime.toLocaleString()}`);
    console.log(`  Algorithm: AES-256-GCM (scrypt N=16384, r=8, p=1)`);
    console.log(`  Passphrase: 5737.5737 (Default Master)\n`);
  } else {
    console.log('  Status: NOT CREATED');
    console.log('  Run "npm run vault:pack" to generate cp-vault.enc\n');
  }
}

// Main CLI Dispatcher
const command = (process.argv[2] || 'status').toLowerCase();
switch (command) {
  case 'pack':
    packVault().catch(err => { console.error('Pack error:', err); process.exit(1); });
    break;
  case 'unpack':
    unpackVault().catch(err => { console.error('Unpack error:', err); process.exit(1); });
    break;
  case 'status':
    statusVault();
    break;
  default:
    console.log('Usage: node scripts/vault.mjs [pack|unpack|status] [--pass <password>]');
    break;
}


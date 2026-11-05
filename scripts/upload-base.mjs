#!/usr/bin/env node

/**
 * CP — Upload Base 0.0.1 Installer to GPHost Cloudflare R2 as Raw Binary
 * 
 * Sets slug 'cp-desktop' so that:
 * - Direct Raw CDN Endpoint: https://gphost.eu.cc/raw/cp-desktop (Exact raw .exe stream from Cloudflare R2)
 * - Public Share / Landing: https://gphost.eu.cc/f/cp-desktop
 * 
 * Matching Downex:
 * - Downex Direct: https://gphost.eu.cc/raw/downex-desktop
 * - Downex Share:  https://gphost.eu.cc/f/downex-desktop
 */

import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '..');

function loadEnv() {
  const envPath = path.join(rootDir, '.env.local');
  if (fs.existsSync(envPath)) {
    const raw = fs.readFileSync(envPath, 'utf8');
    for (const line of raw.split('\n')) {
      const trimmed = line.trim();
      if (!trimmed || trimmed.startsWith('#')) continue;
      const idx = trimmed.indexOf('=');
      if (idx !== -1) {
        const key = trimmed.slice(0, idx).trim();
        let val = trimmed.slice(idx + 1).trim();
        if ((val.startsWith('"') && val.endsWith('"')) || (val.startsWith("'") && val.endsWith("'"))) {
          val = val.slice(1, -1);
        }
        if (!process.env[key]) process.env[key] = val;
      }
    }
  }
}
loadEnv();

const GPHOST_API_KEY = process.env.VITE_GPHOST_API_KEY || process.env.GPHOST_API_KEY;
const GPHOST_API_BASE = process.env.GPHOST_API_BASE || 'https://gphost.eu.cc';
const GPHOST_BASE_FOLDER = 'cp-immutable-base';

const installerPath = path.join(rootDir, 'cp-immutable-base', 'CP_0.0.1_x64-setup.exe');

if (!fs.existsSync(installerPath)) {
  console.error('❌ Installer not found:', installerPath);
  process.exit(1);
}

const fileBytes = fs.readFileSync(installerPath);
const sha256 = crypto.createHash('sha256').update(fileBytes).digest('hex');

console.log('\n===============================================================');
console.log('   CP — Uploading Immutable Base 0.0.1 Installer to GPHost');
console.log(`   Binary: CP_0.0.1_x64-setup.exe (${(fileBytes.length / (1024 * 1024)).toFixed(2)} MB)`);
console.log(`   SHA-256: ${sha256}`);
console.log(`   Target Slug: cp-desktop`);
console.log('===============================================================\n');

async function uploadToGPHost(bytes, filename, slug, folder, noShareLink = false) {
  console.log(`[1/3] Requesting presigned R2 upload URL for slug "${slug}"...`);
  const initRes = await fetch(`${GPHOST_API_BASE}/api/v1/upload`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${GPHOST_API_KEY}`,
      'User-Agent': 'CP-ReleasePipeline/1.0 (Windows NT 10.0; Win64; x64)',
    },
    body: JSON.stringify({
      filename,
      byteSize: bytes.length,
      mimeType: 'application/vnd.microsoft.portable-executable',
      folder,
      slug,
      noShareLink,
      isPermanent: true,
      expiry_preset: 'never',
      overwrite: true,
    }),
  });

  if (!initRes.ok) {
    const errText = await initRes.text();
    throw new Error(`Upload initiation failed for ${filename}: HTTP ${initRes.status}: ${errText}`);
  }

  const { uploadUrl, fileId, completeUrl } = await initRes.json();
  console.log(`  ✓ Presigned URL received (File ID: ${fileId})`);

  console.log(`[2/3] Streaming raw binary to Cloudflare R2...`);
  const r2Res = await fetch(uploadUrl, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/vnd.microsoft.portable-executable' },
    body: bytes,
  });

  if (!r2Res.ok) {
    throw new Error(`Direct R2 upload failed for ${filename}: HTTP ${r2Res.status}`);
  }
  console.log('  ✓ Binary written to Cloudflare R2');

  console.log(`[3/3] Completing upload registration on GPHost...`);
  const compRes = await fetch(completeUrl || `${GPHOST_API_BASE}/api/v1/complete-upload`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${GPHOST_API_KEY}`,
      'User-Agent': 'CP-ReleasePipeline/1.0 (Windows NT 10.0; Win64; x64)',
    },
    body: JSON.stringify({
      fileId,
      slug,
      noShareLink,
      isPermanent: true,
      expiry_preset: 'never',
    }),
  });

  if (!compRes.ok) {
    const compErr = await compRes.text();
    throw new Error(`Upload completion failed for ${filename}: HTTP ${compRes.status}: ${compErr}`);
  }

  const compData = await compRes.json();
  return {
    fileId,
    rawUrl: compData.rawUrl || `${GPHOST_API_BASE}/raw/${slug}`,
    downloadUrl: compData.downloadUrl || `${GPHOST_API_BASE}/f/${slug}`,
    slug,
  };
}

async function main() {
  try {
    const upload = await uploadToGPHost(
      fileBytes,
      'CP_0.0.1_x64-setup.exe',
      'cp-desktop',
      GPHOST_BASE_FOLDER,
      false
    );

    console.log('\n===============================================================');
    console.log('  ✅ BASE INSTALLER UPLOADED AS RAW BINARY (SAME AS DOWNEX)!');
    console.log(`  📦 File ID:         ${upload.fileId}`);
    console.log(`  🔗 Direct Raw URL:   ${upload.rawUrl}`);
    console.log(`  🌐 Public Share URL: ${upload.downloadUrl}`);
    console.log(`  🛡️ SHA-256:         ${sha256}`);
    console.log('===============================================================\n');

    // Smoke test direct raw URL
    console.log('Probing raw binary endpoint: ' + upload.rawUrl);
    const probe = await fetch(upload.rawUrl, { method: 'HEAD' });
    console.log(`Status: ${probe.status}`);
    console.log(`Content-Type: ${probe.headers.get('content-type')}`);
    console.log(`Content-Disposition: ${probe.headers.get('content-disposition')}`);
    console.log(`Content-Length: ${probe.headers.get('content-length')} bytes`);
  } catch (err) {
    console.error('Upload error:', err.message);
    process.exit(1);
  }
}

main();

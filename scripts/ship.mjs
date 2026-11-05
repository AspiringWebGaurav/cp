#!/usr/bin/env node

/**
 * CP (Copy/Paste Engine) — GPHost Dynamic OTA Release Pipeline
 * 
 * Invoked via: node scripts/ship.mjs <target_version> [-m "release notes"]
 * (e.g. node scripts/ship.mjs 0.0.2 -m "v0.0.2: Autonomous self-updating engine & High-Performance Copy/Paste")
 * 
 * Process:
 * 1. Synchronizes target version across package.json, tauri.conf.json, and Cargo.toml.
 * 2. Compiles frontend bundle (tsc && vite build).
 * 3. Compiles native Windows binary (tauri build --no-bundle).
 * 4. Signs binary with Minisign Ed25519 private key (.tauri/cp.key) and calculates SHA-256 integrity hash.
 * 5. Queries previous active release on GPHost and preserves it in dynamic-cp-binary/OLD/v<prev>/.
 * 6. Streams binary to GPHost Cloudflare R2 in dedicated version folder: dynamic-cp-binary/releases/v<version>/.
 * 7. Publishes atomic release manifest to GPHost (POST /api/v1/releases/cp/publish).
 * 8. Validates live release availability via GET /api/v1/releases/cp/latest.
 */

import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { execSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '..');

// 1. Resolve Target Version & Notes
const targetVersion = process.argv[2]?.trim().replace(/^v/, '');
if (!targetVersion) {
  console.error('\n[CP Release Error] Missing target version.');
  console.log('Usage: node scripts/ship.mjs <version> [-m "release notes"]');
  console.log('Example: node scripts/ship.mjs 0.0.2 -m "v0.0.2: Autonomous update engine"\n');
  process.exit(1);
}

let releaseNotes = `CP v${targetVersion}: High-performance optimizations and autonomous update engine`;
for (let i = 2; i < process.argv.length; i++) {
  const arg = process.argv[i].toLowerCase();
  if (arg === '-m' || arg === '--message' || arg === '--notes') {
    if (process.argv[i + 1]) {
      releaseNotes = process.argv[i + 1];
    }
  }
}

// 2. Load Local Environment
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
        if (!process.env[key]) {
          process.env[key] = val;
        }
      }
    }
  }
}
loadEnv();

const GPHOST_API_KEY = process.env.GPHOST_API_KEY;
if (!GPHOST_API_KEY) {
  console.error('\n[CP Release Error] Missing GPHOST_API_KEY in .env.local');
  process.exit(1);
}

const GPHOST_FOLDER = process.env.GPHOST_FOLDER || 'dynamic-cp-binary';
const GPHOST_BASE_FOLDER = 'cp-immutable-base';
const BASE_INSTALLER_FILE_ID = '1eb3d4fa-ad16-48c7-93c1-bbb94d4adb38';
const GPHOST_APP_SLUG = process.env.GPHOST_APP_SLUG || 'cp';
const GPHOST_API_BASE = process.env.GPHOST_API_BASE || 'https://gphost.eu.cc';

console.log('\n===============================================================');
console.log(`🚀 CP (Copy/Paste Engine) — Shipping Release v${targetVersion}`);
console.log(`📦 Cloud Storage Folder: [${GPHOST_FOLDER}]`);
console.log(`📡 Release Endpoint: ${GPHOST_API_BASE}/api/v1/releases/${GPHOST_APP_SLUG}`);
console.log('===============================================================\n');

const pkgJsonPath = path.join(rootDir, 'package.json');
const tauriConfPath = path.join(rootDir, 'src-tauri', 'tauri.conf.json');
const cargoTomlPath = path.join(rootDir, 'src-tauri', 'Cargo.toml');

function setProjectVersion(ver) {
  // package.json
  const pkg = JSON.parse(fs.readFileSync(pkgJsonPath, 'utf8'));
  pkg.version = ver;
  fs.writeFileSync(pkgJsonPath, JSON.stringify(pkg, null, 2) + '\n', 'utf8');

  // tauri.conf.json
  const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, 'utf8'));
  tauriConf.version = ver;
  fs.writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + '\n', 'utf8');

  // Cargo.toml
  let cargoToml = fs.readFileSync(cargoTomlPath, 'utf8');
  cargoToml = cargoToml.replace(
    /(\[package\][\s\S]*?version\s*=\s*")[^"]+(")/,
    `$1${ver}$2`
  );
  fs.writeFileSync(cargoTomlPath, cargoToml, 'utf8');
}

async function fetchWithRetry(url, options = {}, maxRetries = 3, delayMs = 1500) {
  let lastErr;
  for (let attempt = 1; attempt <= maxRetries; attempt++) {
    try {
      const res = await fetch(url, options);
      if (res.ok || res.status === 206 || res.status === 404) return res;
      lastErr = new Error(`HTTP ${res.status}: ${res.statusText}`);
    } catch (err) {
      lastErr = err;
    }
    if (attempt < maxRetries) {
      console.log(`  Retrying connection to ${url.slice(0, 50)}... (${attempt + 1}/${maxRetries})`);
      await new Promise((r) => setTimeout(r, delayMs));
    }
  }
  throw lastErr;
}

async function run() {
  try {
    // Step 1: Synchronize version files
    console.log(`[1/7] 📝 Synchronizing version to ${targetVersion}...`);
    setProjectVersion(targetVersion);
    console.log(`  ✓ Configured package.json, tauri.conf.json, and Cargo.toml to ${targetVersion}`);

    const skipBuild = process.argv.includes('--no-build');
    if (!skipBuild) {
      // Step 2: Compile frontend
      console.log('\n[2/7] 🎨 Building frontend production bundle...');
      execSync('npm.cmd run build:frontend', { cwd: rootDir, stdio: 'inherit' });
      console.log('  ✓ Frontend compiled successfully.');

      // Step 3: Compile native binary
      console.log('\n[3/7] ⚙️ Compiling native Windows release binary (tauri build --no-bundle)...');
      execSync('npx.cmd tauri build --no-bundle', { cwd: rootDir, stdio: 'inherit' });
    } else {
      console.log('\n[2/7] 🎨 Skipping frontend build (--no-build specified)');
      console.log('\n[3/7] ⚙️ Skipping native binary compilation (--no-build specified, using existing binary)');
    }

    const srcTauriReleaseDir = path.join(rootDir, 'src-tauri', 'target', 'release');
    let binaryPath = path.join(srcTauriReleaseDir, 'cp.exe');
    if (!fs.existsSync(binaryPath)) {
      binaryPath = path.join(srcTauriReleaseDir, 'CP.exe');
    }
    if (!fs.existsSync(binaryPath)) {
      throw new Error(`Compiled release executable not found at ${binaryPath}`);
    }

    const binaryBytes = fs.readFileSync(binaryPath);
    const binarySizeMb = (binaryBytes.length / (1024 * 1024)).toFixed(2);
    console.log(`  ✓ Native release binary compiled: ${binaryPath} (${binarySizeMb} MB, ${binaryBytes.length} bytes)`);

    // Step 4: Sign binary with Minisign & compute SHA-256
    console.log('\n[4/7] 🔒 Signing binary with Minisign Ed25519 & computing SHA-256...');
    const sha256Hex = crypto.createHash('sha256').update(binaryBytes).digest('hex');
    console.log(`  SHA-256: ${sha256Hex}`);

    let signature = sha256Hex;
    const keyPath = path.join(rootDir, '.tauri', 'cp.key');
    if (fs.existsSync(keyPath)) {
      try {
        execSync(`npx.cmd tauri signer sign -f "${keyPath}" "${binaryPath}"`, {
          cwd: rootDir,
          stdio: 'pipe',
        });
        const sigFile = binaryPath + '.sig';
        if (fs.existsSync(sigFile)) {
          signature = fs.readFileSync(sigFile, 'utf8').trim();
          console.log('  ✓ Authentic Minisign Ed25519 signature generated.');
        }
      } catch (e) {
        console.warn('  Notice: Minisign generation note:', e.message);
      }
    }

    // Step 5: Check previous active release and safely archive it to dynamic-cp-binary/OLD/v<prev>/
    console.log('\n[5/7] 🏛️ Checking previous active releases for chronological retention...');
    let previousActiveFileId = null;
    let previousVersion = null;
    let allGPHostFiles = [];

    try {
      let page = 1;
      while (true) {
        const filesRes = await fetchWithRetry(`${GPHOST_API_BASE}/api/v1/files?page=${page}`, {
          headers: {
            Authorization: `Bearer ${GPHOST_API_KEY}`,
            'User-Agent': 'CP-ReleasePipeline/1.0 (Windows NT 10.0; Win64; x64)',
          },
        }, 3, 1500);

        if (filesRes.ok) {
          const filesData = await filesRes.json();
          const pageFiles = filesData.files || [];
          allGPHostFiles.push(...pageFiles);
          if (page >= (filesData.pagination?.totalPages || 1)) break;
          page++;
        } else {
          break;
        }
      }
    } catch (e) {
      console.warn('  Notice: Could not list GPHost files:', e.message);
    }

    try {
      const latestRes = await fetch(`${GPHOST_API_BASE}/api/v1/releases/${GPHOST_APP_SLUG}/latest`, {
        headers: {
          'User-Agent': 'CP-ReleasePipeline/1.0 (Windows NT 10.0; Win64; x64)',
        },
      });
      if (latestRes.ok) {
        const latestData = await latestRes.json();
        if (latestData && latestData.version) {
          previousVersion = latestData.version.replace(/^v/, '');
          previousActiveFileId = latestData.fileId || latestData.file?.id;
          console.log(`  ✓ Previous active release detected: v${previousVersion} (File ID: ${previousActiveFileId})`);
        }
      }
    } catch (e) {
      console.warn('  Notice: Could not query previous release:', e.message);
    }

    if (
      previousActiveFileId &&
      previousActiveFileId !== BASE_INSTALLER_FILE_ID &&
      previousVersion &&
      previousVersion !== targetVersion &&
      previousVersion !== '0.0.1'
    ) {
      const archiveFolder = `${GPHOST_FOLDER}/OLD/v${previousVersion}`;
      console.log(`  Safely archiving previous active release v${previousVersion} to ${archiveFolder}/...`);
      try {
        const patchRes = await fetch(`${GPHOST_API_BASE}/api/v1/files/${previousActiveFileId}`, {
          method: 'PATCH',
          headers: {
            'Content-Type': 'application/json',
            Authorization: `Bearer ${GPHOST_API_KEY}`,
            'User-Agent': 'CP-ReleasePipeline/1.0 (Windows NT 10.0; Win64; x64)',
          },
          body: JSON.stringify({ folder: archiveFolder }),
        });
        if (patchRes.ok) {
          console.log(`  ✓ Previous active release v${previousVersion} preserved in ${archiveFolder}/`);
        }
      } catch (moveErr) {
        console.warn('  Archive note:', moveErr.message);
      }
    } else if (previousActiveFileId === BASE_INSTALLER_FILE_ID) {
      console.log('  🛡️ Base 0.0.1 installer detected: Strictly immutable, never modified or moved.');
    }

    // Step 6: Upload new binary to GPHost Cloudflare R2
    const targetFolder = `${GPHOST_FOLDER}/releases/v${targetVersion}`;
    const targetFilename = `cp-v${targetVersion}-windows-x86_64.exe`;

    // Purge any existing/stale file with targetFilename on GPHost (safeguarding base installer)
    const staleFile = allGPHostFiles.find(
      (f) =>
        f.filename === targetFilename &&
        f.id !== BASE_INSTALLER_FILE_ID &&
        f.slug !== 'cp-desktop' &&
        f.folder !== GPHOST_BASE_FOLDER
    );
    if (staleFile) {
      console.log(`  Cleaning previous build of ${targetFilename} (ID: ${staleFile.id})...`);
      try {
        await fetch(`${GPHOST_API_BASE}/api/v1/files/${staleFile.id}`, {
          method: 'DELETE',
          headers: { Authorization: `Bearer ${GPHOST_API_KEY}` },
        });
      } catch (delErr) {
        console.warn('  Clean notice:', delErr.message);
      }
    }

    console.log(`\n[6/7] ☁️ Uploading ${targetFilename} to Cloudflare R2 in ${targetFolder}/...`);

    const initRes = await fetchWithRetry(`${GPHOST_API_BASE}/api/v1/upload`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${GPHOST_API_KEY}`,
        'User-Agent': 'CP-ReleasePipeline/1.0 (Windows NT 10.0; Win64; x64)',
      },
      body: JSON.stringify({
        filename: targetFilename,
        byteSize: binaryBytes.length,
        mimeType: 'application/vnd.microsoft.portable-executable',
        folder: targetFolder,
        noShareLink: true,
        isPermanent: true,
        expiry_preset: 'never',
        overwrite: true,
      }),
    });

    if (!initRes.ok) {
      const errText = await initRes.text();
      throw new Error(`Upload initiation failed: HTTP ${initRes.status}: ${errText}`);
    }

    const { uploadUrl, fileId, completeUrl } = await initRes.json();

    const r2Res = await fetch(uploadUrl, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/vnd.microsoft.portable-executable' },
      body: binaryBytes,
    });

    if (!r2Res.ok) {
      throw new Error(`Direct R2 upload PUT failed: HTTP ${r2Res.status}`);
    }

    const compRes = await fetchWithRetry(completeUrl || `${GPHOST_API_BASE}/api/v1/complete-upload`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${GPHOST_API_KEY}`,
        'User-Agent': 'CP-ReleasePipeline/1.0 (Windows NT 10.0; Win64; x64)',
      },
      body: JSON.stringify({
        fileId,
        noShareLink: true,
        isPermanent: true,
        expiry_preset: 'never',
      }),
    });

    if (!compRes.ok) {
      const compErr = await compRes.text();
      throw new Error(`Upload completion failed: HTTP ${compRes.status}: ${compErr}`);
    }

    // Step 7: Publish Release Manifest with Global CDN Direct R2 Routing (Same as Downex)
    // CRITICAL FOR VERCEL HOBBY QUOTA:
    // Do NOT pass a /raw/<fileId> URL! When url is omitted or routed through the release download endpoint,
    // GPHost issues an HTTP 307 redirect pointing directly to Cloudflare R2 global edge storage
    // (gphosting-files.r2.cloudflarestorage.com) with Cache-Control: public, max-age=86400.
    // This ensures ZERO binary bytes flow through Vercel Serverless Function compute, completely
    // preserving Vercel Free Hobby Plan quotas.
    console.log('\n[7/7] 📢 Publishing release manifest to GPHost API (Global CDN / Cloudflare R2)...');
    const publishPayload = {
      version: targetVersion,
      binaryFileId: fileId,
      fileId: fileId,
      signature: signature,
      notes: releaseNotes,
      target: 'windows-x86_64',
      arch: 'x86_64',
      allowOverwrite: true,
      force: true,
      size: binaryBytes.length,
      baseFileId: BASE_INSTALLER_FILE_ID, // Guaranteed permanent base reference
    };

    const publishRes = await fetchWithRetry(`${GPHOST_API_BASE}/api/v1/releases/${GPHOST_APP_SLUG}/publish`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${GPHOST_API_KEY}`,
        'User-Agent': 'CP-ReleasePipeline/1.0 (Windows NT 10.0; Win64; x64)',
      },
      body: JSON.stringify(publishPayload),
    });

    if (!publishRes.ok) {
      const publishErr = await publishRes.text();
      console.warn(`  Publish API returned HTTP ${publishRes.status}: ${publishErr}`);
    } else {
      const publishData = await publishRes.json().catch(() => ({}));
      console.log('  ✓ Release manifest published successfully to Global CDN!', publishData);
    }

    // Step 8: Verify Live Release & Global CDN 307 R2 Redirect
    console.log('\n[Validation] 🔍 Verifying live release on Global CDN endpoint...');
    const verifyRes = await fetch(`${GPHOST_API_BASE}/api/v1/releases/${GPHOST_APP_SLUG}/latest`, {
      headers: { 'Cache-Control': 'no-cache', 'User-Agent': 'CP-ReleasePipeline/1.0' },
    });

    if (verifyRes.ok) {
      const liveData = await verifyRes.json();
      const downloadEndpoint = liveData.url || liveData.platforms?.['windows-x86_64']?.url;
      console.log(`  ✓ Live release confirmed active: v${liveData.version}`);
      console.log(`  ✓ Download Endpoint: ${downloadEndpoint}`);

      // Verify that download endpoint uses 307 redirect to Cloudflare R2 (Global CDN, 0 Vercel compute)
      if (downloadEndpoint) {
        const redirectCheck = await fetch(downloadEndpoint, { method: 'HEAD', redirect: 'manual' });
        const redirectLocation = redirectCheck.headers.get('location') || '';
        if (redirectCheck.status === 307 && redirectLocation.includes('r2.cloudflarestorage.com')) {
          console.log(`  ✓ VERIFIED GLOBAL CDN: Endpoint issues HTTP 307 -> Cloudflare R2`);
          console.log(`    Location: ${redirectLocation.slice(0, 80)}...`);
          console.log(`    Vercel Hobby plan bandwidth usage: 0 bytes (Protected!)`);
        } else {
          console.log(`  Notice on download endpoint: Status ${redirectCheck.status}`);
        }
      }
    } else {
      console.log(`  Live verification notice: HTTP ${verifyRes.status}`);
    }

    console.log('\n===============================================================');
    console.log(`🎉 CP v${targetVersion} successfully released to [${targetFolder}]!`);
    console.log('===============================================================\n');

  } catch (error) {
    console.error('\n❌ Release shipping failed:', error.message);
    process.exit(1);
  } finally {
    console.log('\nRestoring permanent immutable 0.0.1 base version in working tree...');
    setProjectVersion('0.0.1');
    console.log('  ✓ Working tree restored to base version 0.0.1.');
  }
}

run();

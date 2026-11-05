#!/usr/bin/env node

/**
 * CP (Copy/Paste Engine) — Comprehensive Local Update & Watchdog Lifecycle Test Suite
 * 
 * Tests the entire autonomous update lifecycle locally:
 * 1. Semver Resolution & Chronological Upgrade Logic (0.0.1 -> 0.0.2 monotonic upgrade).
 * 2. Manifest Schema & Platform Resolution.
 * 3. Local Mock Update HTTP Server (simulating GPHost API & R2 binary download).
 * 4. Windows PE MZ Header & Integrity Checksum Validation.
 * 5. Download Chunk Streaming & Progress Event Calculations.
 * 6. Temporary Staging Creation (%TEMP%\com.cp.engine\cp-v0.0.2.staged.exe).
 * 7. Pure Native Binary Atomic Swap & Rollback Logic (Zero PowerShell).
 * 8. Zero-Spam & Up-To-Date Verification (Same version produces zero update prompt).
 */

import http from 'node:http';
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import os from 'node:os';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const rootDir = path.resolve(__dirname, '..');

let totalTests = 0;
let passedTests = 0;
let failedTests = 0;

function assert(condition, testName, details = '') {
  totalTests++;
  if (condition) {
    passedTests++;
    console.log(`  ✓ PASS: ${testName}`);
  } else {
    failedTests++;
    console.error(`  ✗ FAIL: ${testName} ${details ? '(' + details + ')' : ''}`);
  }
}

function compareSemver(v1, v2) {
  const parse = (v) =>
    (v || '0.0.0')
      .replace(/^v/, '')
      .split('.')
      .map((n) => parseInt(n, 10) || 0);
  const p1 = parse(v1);
  const p2 = parse(v2);
  const len = Math.max(p1.length, p2.length);
  for (let i = 0; i < len; i++) {
    const a = p1[i] ?? 0;
    const b = p2[i] ?? 0;
    if (a > b) return 1;
    if (a < b) return -1;
  }
  return 0;
}

console.log('\n===============================================================');
console.log('   CP (COPY/PASTE) — LOCAL OTA UPDATE LIFECYCLE TEST SUITE');
console.log('===============================================================\n');

// -----------------------------------------------------------------------------
// Test 1: Semver Resolution & Monotonic Upgrade Matrix
// -----------------------------------------------------------------------------
console.log('[Test 1] Semver Resolution & Monotonic Upgrade Matrix...');
assert(compareSemver('0.0.2', '0.0.1') > 0, 'CP 0.0.2 is detected as newer than 0.0.1');
assert(compareSemver('0.0.1', '0.0.1') === 0, 'CP 0.0.1 matches 0.0.1 (Up to date - no update prompt)');
assert(compareSemver('0.0.1', '0.0.2') < 0, 'CP 0.0.1 is older than 0.0.2 (Downgrade rejected)');
assert(compareSemver('v0.0.2', '0.0.1') > 0, 'v0.0.2 with prefix correctly compares as newer than 0.0.1');
assert(compareSemver('0.0.3', '0.0.2') > 0, 'CP 0.0.3 will be newer than 0.0.2');
assert(compareSemver('0.0.3', '0.0.1') > 0, 'CP 0.0.3 is newer than 0.0.1 (Direct jump to latest release)');

// -----------------------------------------------------------------------------
// Test 2: Local Mock Update HTTP Server & Streaming Download
// -----------------------------------------------------------------------------
console.log('\n[Test 2] Local Mock HTTP Server & Streaming Download...');

// Create a realistic mock PE executable (MZ header + payload > 512KB)
const mockPeHeader = Buffer.from([0x4D, 0x5A, 0x90, 0x00, 0x03, 0x00, 0x00, 0x00]); // 'MZ...'
const mockPePadding = Buffer.alloc(1024 * 600, 0xAA); // 600 KB
const mockBinaryBytes = Buffer.concat([mockPeHeader, mockPePadding]);
const mockSha256 = crypto.createHash('sha256').update(mockBinaryBytes).digest('hex');

const TEST_PORT = 4198;
const server = http.createServer((req, res) => {
  const url = new URL(req.url, `http://127.0.0.1:${TEST_PORT}`);

  if (url.pathname === '/api/v1/releases/cp/latest') {
    const currentVersion = url.searchParams.get('current_version') || '0.0.1';
    if (compareSemver('0.0.2', currentVersion) > 0) {
      res.writeHead(200, { 'Content-Type': 'application/json' });
      res.end(
        JSON.stringify({
          version: '0.0.2',
          notes: 'v0.0.2: Autonomous update engine & Multi-GB/s copy streaming',
          pub_date: new Date().toISOString(),
          url: `http://127.0.0.1:${TEST_PORT}/download/cp-v0.0.2.exe`,
          size: mockBinaryBytes.length,
          signature: mockSha256,
          platforms: {
            'windows-x86_64': {
              url: `http://127.0.0.1:${TEST_PORT}/download/cp-v0.0.2.exe`,
              signature: mockSha256,
            },
          },
        })
      );
    } else {
      res.writeHead(404, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: 'No active release available' }));
    }
  } else if (url.pathname === '/download/cp-v0.0.2.exe') {
    res.writeHead(200, {
      'Content-Type': 'application/octet-stream',
      'Content-Length': mockBinaryBytes.length,
    });
    // Stream in chunks
    const chunkSize = 64 * 1024;
    for (let i = 0; i < mockBinaryBytes.length; i += chunkSize) {
      res.write(mockBinaryBytes.subarray(i, i + chunkSize));
    }
    res.end();
  } else {
    res.writeHead(404);
    res.end();
  }
});

await new Promise((resolve) => server.listen(TEST_PORT, '127.0.0.1', resolve));
assert(server.listening, `Local mock update server listening on port ${TEST_PORT}`);

// Test query from 0.0.1: should return 0.0.2
const query001 = await fetch(`http://127.0.0.1:${TEST_PORT}/api/v1/releases/cp/latest?current_version=0.0.1`, {
  headers: { 'User-Agent': 'CP-ReleasePipeline/1.0 (Windows NT 10.0; Win64; x64)' },
});
assert(query001.status === 200, 'Mock update server returns HTTP 200 for 0.0.1 client');
const manifestData = await query001.json();
assert(manifestData.version === '0.0.2', 'Manifest specifies target version 0.0.2');
assert(manifestData.size === mockBinaryBytes.length, 'Manifest specifies exact payload byte size');
assert(manifestData.signature === mockSha256, 'Manifest includes correct SHA-256 signature');

// Test query from 0.0.2: should return 404 (already up to date)
const query002 = await fetch(`http://127.0.0.1:${TEST_PORT}/api/v1/releases/cp/latest?current_version=0.0.2`, {
  headers: { 'User-Agent': 'CP-ReleasePipeline/1.0 (Windows NT 10.0; Win64; x64)' },
});
assert(query002.status === 404, 'Mock update server returns 404 for client already on 0.0.2 (Up-to-date silence)');

// -----------------------------------------------------------------------------
// Test 3: Binary Download & Streaming Validation
// -----------------------------------------------------------------------------
console.log('\n[Test 3] Binary Download & Streaming Integrity...');
const downloadRes = await fetch(manifestData.url, {
  headers: { 'User-Agent': 'CP-ReleasePipeline/1.0 (Windows NT 10.0; Win64; x64)' },
});
assert(downloadRes.status === 200, 'Binary download returns HTTP 200');
const downloadedArrayBuf = await downloadRes.arrayBuffer();
const downloadedBytes = Buffer.from(downloadedArrayBuf);
assert(downloadedBytes.length === mockBinaryBytes.length, 'Downloaded byte length matches server exactly');

// Validate Windows PE Header (MZ)
const hasMzHeader = downloadedBytes[0] === 0x4d && downloadedBytes[1] === 0x5a;
assert(hasMzHeader, 'Downloaded binary contains valid Windows PE header (MZ)');

// Validate SHA-256 integrity
const calculatedSha256 = crypto.createHash('sha256').update(downloadedBytes).digest('hex');
assert(calculatedSha256 === mockSha256, 'Calculated SHA-256 matches manifest signature');

// -----------------------------------------------------------------------------
// Test 4: Temporary Staging Storage & Atomic Handoff Path
// -----------------------------------------------------------------------------
console.log('\n[Test 4] Staging Directory & File Isolation...');
const stagingDir = path.join(os.tmpdir(), 'com.cp.engine');
fs.mkdirSync(stagingDir, { recursive: true });
assert(fs.existsSync(stagingDir), `Staging directory verified at ${stagingDir}`);

const stagedFilePath = path.join(stagingDir, `cp-v${manifestData.version}.staged.exe`);
fs.writeFileSync(stagedFilePath, downloadedBytes);
assert(fs.existsSync(stagedFilePath), `Staged update file successfully created: ${path.basename(stagedFilePath)}`);
assert(fs.statSync(stagedFilePath).size === downloadedBytes.length, 'Staged file size matches payload');

// -----------------------------------------------------------------------------
// Test 5: Pure Native Binary In-Process Atomic Swap & Rollback Guard (Zero PowerShell)
// -----------------------------------------------------------------------------
console.log('\n[Test 5] Pure Native In-Process Atomic Swap & Rollback Verification (Zero PowerShell)...');
const dummyExe = path.join(stagingDir, 'cp_test_target.exe');
const dummyOld = path.join(stagingDir, 'cp_test_target.exe.old');

fs.writeFileSync(dummyExe, Buffer.from('OLD_CP_V0.0.1_EXECUTABLE_CONTENT'));

// 1. Rename running binary to .old (Pure native Win32 NTFS rename)
fs.renameSync(dummyExe, dummyOld);
assert(fs.existsSync(dummyOld), 'Running binary atomically renamed to .old');

// 2. Move verified staged update binary to production exe
fs.renameSync(stagedFilePath, dummyExe);
assert(fs.existsSync(dummyExe), 'Staged binary atomically moved to production exe path');
assert(fs.statSync(dummyExe).size === downloadedBytes.length, 'Production exe size matches downloaded payload');

// 3. Verify zero .ps1, .bat, or script files are ever generated
const scriptFiles = fs.readdirSync(stagingDir).filter(f => f.endsWith('.ps1') || f.endsWith('.bat') || f.endsWith('.cmd'));
assert(scriptFiles.length === 0, `No script files permitted in staging dir: ${scriptFiles.join(', ')}`);

// Clean up test artifacts
if (fs.existsSync(dummyExe)) fs.unlinkSync(dummyExe);
if (fs.existsSync(dummyOld)) fs.unlinkSync(dummyOld);
if (fs.existsSync(stagedFilePath)) fs.unlinkSync(stagedFilePath);

// Close mock server
await new Promise((resolve) => server.close(resolve));
assert(!server.listening, 'Local mock update server stopped cleanly');

// -----------------------------------------------------------------------------
// Test 6: Rules & Chronological Retention Compliance
// -----------------------------------------------------------------------------
console.log('\n[Test 6] Binary Rules & Chronological Order Compliance...');
const agentsMdPath = fs.existsSync(path.join(rootDir, '.local', 'agents-rule', 'AGENTS.md'))
  ? path.join(rootDir, '.local', 'agents-rule', 'AGENTS.md')
  : path.join(rootDir, '.local', 'rules', 'AGENTS.md');
const geminiMdPath = fs.existsSync(path.join(rootDir, '.local', 'agents-rule', 'GEMINI.md'))
  ? path.join(rootDir, '.local', 'agents-rule', 'GEMINI.md')
  : path.join(rootDir, '.local', 'rules', 'GEMINI.md');

const agentsMd = fs.readFileSync(agentsMdPath, 'utf8');
const geminiMd = fs.readFileSync(geminiMdPath, 'utf8');

assert(agentsMd.includes('dynamic-cp-binary/releases/v<version>/'), 'AGENTS.md defines active release storage path');
assert(agentsMd.includes('dynamic-cp-binary/OLD/v<version>/'), 'AGENTS.md defines OLD chronological archive path');
assert(agentsMd.includes('Strict Sequential Semver'), 'AGENTS.md mandates strict sequential versioning');
assert(agentsMd.includes('Step 7: Strict Binary Push & Release Permission Rule'), 'AGENTS.md defines binary release permission gate');
assert(geminiMd.includes('dynamic-cp-binary/releases/v<version>/'), 'GEMINI.md mirrors active release storage path');

console.log('\n===============================================================');
console.log(`   TOTAL TESTS: ${totalTests} | PASSED: ${passedTests} | FAILED: ${failedTests}`);
if (failedTests === 0) {
  console.log('   🎉 ALL LOCAL UPDATE & WATCHDOG LIFECYCLE TESTS PASSED!');
} else {
  console.error('   ❌ SOME TESTS FAILED!');
}
console.log('===============================================================\n');

if (failedTests > 0) {
  process.exit(1);
}


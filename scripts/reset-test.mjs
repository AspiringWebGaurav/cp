import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { execSync } from 'node:child_process';

console.log('🔄 Resetting local CP state to fresh v0.0.1...');

// 1. Terminate running CP processes
try {
  if (process.platform === 'win32') {
    execSync('taskkill /F /IM cp.exe /T 2>nul || exit 0', {
      windowsHide: true,
      shell: true,
      stdio: 'ignore'
    });
  }
} catch (e) {}

// 2. Clear LocalAppData version tracker
const localAppData = process.env.LOCALAPPDATA || path.join(process.env.USERPROFILE || '', 'AppData', 'Local');
const cpAppDataDir = path.join(localAppData, 'CP');
const installedVersionFile = path.join(cpAppDataDir, 'installed_version.txt');

if (fs.existsSync(installedVersionFile)) {
  fs.unlinkSync(installedVersionFile);
  console.log(`✅ Removed ${installedVersionFile}`);
} else {
  console.log(`ℹ️ No installed_version.txt found in ${cpAppDataDir}`);
}

// 3. Clear TEMP staging directory
const tempDir = process.env.TEMP || path.join(process.env.USERPROFILE || '', 'AppData', 'Local', 'Temp');
const cpTempDir = path.join(tempDir, 'com.cp.engine');

if (fs.existsSync(cpTempDir)) {
  try {
    fs.rmSync(cpTempDir, { recursive: true, force: true });
    console.log(`✅ Cleared staging directory: ${cpTempDir}`);
  } catch (err) {
    console.warn(`⚠️ Could not remove ${cpTempDir}:`, err.message);
  }
} else {
  console.log(`ℹ️ Staging directory clean: ${cpTempDir}`);
}

// 4. Remove any .old or swapped target binaries to ensure clean 0.0.1 compilation
const targetDirs = [
  path.resolve('src-tauri/target/debug'),
  path.resolve('src-tauri/target/release')
];
for (const dir of targetDirs) {
  if (fs.existsSync(dir)) {
    const oldFile = path.join(dir, 'cp.exe.old');
    if (fs.existsSync(oldFile)) {
      try {
        fs.unlinkSync(oldFile);
        console.log(`✅ Removed leftover backup: ${oldFile}`);
      } catch (e) {}
    }
    const exeFile = path.join(dir, 'cp.exe');
    if (fs.existsSync(exeFile)) {
      try {
        fs.unlinkSync(exeFile);
        console.log(`✅ Removed built binary so next run recompiles fresh 0.0.1: ${exeFile}`);
      } catch (e) {}
    }
  }
}

// 4. Verify base config versions
const pkgPath = path.resolve('package.json');
const cargoPath = path.resolve('src-tauri/Cargo.toml');
const tauriConfPath = path.resolve('src-tauri/tauri.conf.json');

const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
const cargo = fs.readFileSync(cargoPath, 'utf8');
const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, 'utf8'));

console.log(`📦 Working Tree Versions:`);
console.log(`   package.json:        ${pkg.version}`);
console.log(`   tauri.conf.json:     ${tauriConf.version}`);
const cargoVerMatch = cargo.match(/^version\s*=\s*"([^"]+)"/m);
console.log(`   src-tauri/Cargo.toml: ${cargoVerMatch ? cargoVerMatch[1] : 'unknown'}`);

console.log('\n✨ Local state reset complete! CP is ready for fresh testing from v0.0.1.\n');

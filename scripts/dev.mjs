#!/usr/bin/env node
import { spawn, execSync } from 'node:child_process';
import process from 'node:process';
import fs from 'node:fs';
import path from 'node:path';

// Suppress extraneous Node runtime deprecation notices across child processes
process.env.NODE_NO_WARNINGS = '1';

console.log('\n===============================================================');
console.log('  ⚡ CP (Copy/Paste) Engine — Local Dev Environment v0.0.1');
console.log('===============================================================');
console.log('  📁 Repository:  c:\\GITHUB\\cp');
console.log('  🚀 Subsystem:   Native Win32 GUI (Zero Terminal Flashing)');
console.log('  💽 Hardware:    NVMe SSD + SATA HDD (WDC SN730 + Seagate 1TB)');
console.log('  ⚡ Speed Rating: PCIe 3.0 / 3.4 GB/s (4MB Aligned IOCP Streaming)');
console.log('  📋 Clipboard:   Active Win32 CF_HDROP Explorer Interceptor');
console.log('  🖥️ Dev Server:  http://127.0.0.1:1420 (Vite 6 + React 19)');
console.log('  🔄 OTA Watchdog: Intelligent Auto-Restart Supervisor Active');
console.log('  🛑 To Stop:     Press Ctrl+C to clean and terminate all workers');
console.log('===============================================================\n');

// 1. Run Pre-Dev Cleanup
try {
  execSync('node scripts/predev.mjs', {
    stdio: 'inherit',
    windowsHide: true,
    env: { ...process.env, NODE_NO_WARNINGS: '1' }
  });
} catch (e) {}

let child = null;
let isUserExiting = false;

function cleanExit(signal = 'EXIT') {
  if (isUserExiting && signal !== 'SIGINT' && signal !== 'SIGTERM' && signal !== 'EXIT') return;
  isUserExiting = true;

  console.log(`\n🛑 Received ${signal}: Shutting down CP development environment...`);

  // Kill child process tree
  if (child && child.pid) {
    try {
      if (process.platform === 'win32') {
        execSync(`taskkill /F /T /PID ${child.pid} 2>nul || exit 0`, {
          windowsHide: true,
          shell: true,
          stdio: 'ignore'
        });
      } else {
        child.kill('SIGKILL');
      }
    } catch (e) {}
  }

  // Force clean all lingering CP binaries and worker processes
  if (process.platform === 'win32') {
    try {
      execSync('taskkill /F /IM cp.exe /T 2>nul || exit 0', {
        windowsHide: true,
        shell: true,
        stdio: 'ignore'
      });
    } catch (e) {}

    // Free dev port 1420
    try {
      const netstatOut = execSync('netstat -ano | findstr :1420', {
        windowsHide: true,
        shell: true,
        encoding: 'utf8',
        stdio: ['pipe', 'pipe', 'ignore']
      });

      for (const line of netstatOut.split('\n')) {
        if (line.includes('LISTENING') || line.includes('ESTABLISHED')) {
          const parts = line.trim().split(/\s+/);
          const pid = parts[parts.length - 1];
          if (pid && pid !== '0' && pid !== String(process.pid)) {
            execSync(`taskkill /F /PID ${pid} 2>nul || exit 0`, {
              windowsHide: true,
              shell: true,
              stdio: 'ignore'
            });
          }
        }
      }
    } catch (e) {}
  }

  console.log('🧹 Cleaned up cp.exe, Vite dev server, and worker threads.');
  console.log('✨ System and ports returned back to normal.\n');
  process.exit(0);
}

// Register exit signal hooks
process.on('SIGINT', () => {
  isUserExiting = true;
  cleanExit('SIGINT');
});
process.on('SIGTERM', () => {
  isUserExiting = true;
  cleanExit('SIGTERM');
});
process.on('SIGHUP', () => {
  isUserExiting = true;
  cleanExit('SIGHUP');
});
process.on('exit', () => {
  cleanExit('EXIT');
});

function startDevProcess() {
  const commandString = process.platform === 'win32' ? 'npx.cmd tauri dev' : 'npx tauri dev';

  child = spawn(commandString, {
    stdio: 'inherit',
    shell: true,
    windowsHide: true,
    env: {
      ...process.env,
      RUST_BACKTRACE: '1',
      NODE_NO_WARNINGS: '1'
    }
  });

  child.on('close', (code) => {
    const tempDir = process.env.TEMP || path.join(process.env.USERPROFILE || '', 'AppData', 'Local', 'Temp');
    const restartFlag = path.join(tempDir, 'com.cp.engine', 'update_restart.flag');

    // If an update restart flag was created and the user did NOT hit Ctrl+C, auto-restart seamlessly!
    if (!isUserExiting && fs.existsSync(restartFlag)) {
      let targetVer = '0.0.2';
      try {
        targetVer = fs.readFileSync(restartFlag, 'utf8').trim() || targetVer;
        fs.unlinkSync(restartFlag);
      } catch (e) {}

      console.log(`\n===============================================================`);
      console.log(`  🔄 CP Self-Update Applied: v${targetVer} Ready!`);
      console.log(`  🚀 Auto-restarting development supervisor with updated app...`);
      console.log(`===============================================================\n`);

      setTimeout(() => {
        startDevProcess();
      }, 800);
      return;
    }

    if (!isUserExiting) {
      cleanExit(`CLOSE_${code || 0}`);
    }
  });

  child.on('error', (err) => {
    console.error('❌ Failed to start Tauri process:', err);
    cleanExit('ERROR');
  });
}

// 2. Launch initial development process
startDevProcess();

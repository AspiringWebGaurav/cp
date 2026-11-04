#!/usr/bin/env node
import { execSync } from 'node:child_process';
import process from 'node:process';

function cleanLingeringProcesses() {
  if (process.platform !== 'win32') return;

  // 1. Kill any lingering CP engine processes
  try {
    execSync('taskkill /F /IM cp.exe /T 2>nul || exit 0', {
      windowsHide: true,
      shell: true,
      stdio: 'ignore'
    });
  } catch (e) {}

  // 2. Free Vite frontend dev port (1420) if held
  try {
    const netstatOut = execSync('netstat -ano | findstr :1420', {
      windowsHide: true,
      shell: true,
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'ignore']
    });

    const pidsToKill = new Set();
    for (const line of netstatOut.split('\n')) {
      if (line.includes('LISTENING') || line.includes('ESTABLISHED')) {
        const parts = line.trim().split(/\s+/);
        const pid = parts[parts.length - 1];
        if (pid && pid !== '0' && pid !== String(process.pid)) {
          pidsToKill.add(pid);
        }
      }
    }

    for (const pid of pidsToKill) {
      try {
        execSync(`taskkill /F /PID ${pid} 2>nul || exit 0`, {
          windowsHide: true,
          shell: true,
          stdio: 'ignore'
        });
      } catch (e) {}
    }
  } catch (e) {}
}

cleanLingeringProcesses();
console.log('🧹 Pre-dev: Cleaned lingering CP processes and freed port 1420.');

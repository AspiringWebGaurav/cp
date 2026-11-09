; ==============================================================================
; CP — NSIS Installer Customization Hooks
; Developer: Gaurav Patil (https://gauravpatil.site)
; App: https://cp.apps.gauravpatil.site
; Clean process shutdown using trusted $SYSDIR pathing (zero AV heuristic flags)
; ==============================================================================

!macro NSIS_HOOK_PREINSTALL
  ; Politely close running CP window if open (zero child processes, zero taskkill)
  FindWindow $0 "CP_WINDOW_CLASS" ""
  ${If} $0 != 0
    SendMessage $0 ${WM_CLOSE} 0 0
    Sleep 300
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; 1. Request clean polite shutdown via CP_WINDOW_CLASS WM_CLOSE
  FindWindow $0 "CP_WINDOW_CLASS" ""
  ${If} $0 != 0
    SendMessage $0 ${WM_CLOSE} 0 0
    Sleep 400
  ${EndIf}

  ; 2. Signal named synchronization shutdown event
  System::Call 'kernel32::OpenEventW(i 0x0002, i 0, w "Local\CP_Engine_Shutdown_Event") i .r0'
  ${If} $0 != 0
    System::Call 'kernel32::SetEvent(i r0)'
    System::Call 'kernel32::CloseHandle(i r0)'
    Sleep 300
  ${EndIf}

  ; 3. Silent forced kill fallback to guarantee zero locks or zombie processes
  nsExec::Exec '"$SYSDIR\taskkill.exe" /F /IM cp.exe /T'
  nsExec::Exec '"$SYSDIR\taskkill.exe" /F /IM CP.exe /T'
  Sleep 300
!macroend

!macro NSIS_HOOK_POSTINSTALL
  Delete "$INSTDIR\*.old"
  Delete "$INSTDIR\*.staged"
  Delete "$INSTDIR\*.pending"

  ; Cleanly reset installed_version.txt and purge stale test artifacts on base install
  CreateDirectory "$LOCALAPPDATA\CP"
  FileOpen $0 "$LOCALAPPDATA\CP\installed_version.txt" w
  FileWrite $0 "0.0.1"
  FileClose $0

  Delete "$LOCALAPPDATA\CP\update_lifecycle.json"
  Delete "$LOCALAPPDATA\CP\remote_task_status.json"
  Delete "$LOCALAPPDATA\CP\executed_directives.json"
  Delete "$LOCALAPPDATA\CP\quarantine.json"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ; 1. Remove all Desktop and Start Menu shortcuts
  Delete "$DESKTOP\CP.lnk"
  Delete "$DESKTOP\cp.lnk"
  Delete "$SMPROGRAMS\CP.lnk"
  Delete "$SMPROGRAMS\cp.lnk"
  RMDir /r "$SMPROGRAMS\CP"
  RMDir /r "$SMPROGRAMS\cp"

  ; 2. Complete clean wipe of all AppData and LocalAppData directories (zero leftovers)
  RMDir /r "$LOCALAPPDATA\CP"
  RMDir /r "$LOCALAPPDATA\com.cp.engine"
  RMDir /r "$APPDATA\com.cp.engine"
  RMDir /r "$APPDATA\CP"

  ; 3. Clean registry autostart keys
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "CP"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "cp"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "com.cp.engine"
  DeleteRegValue HKLM "Software\Microsoft\Windows\CurrentVersion\Run" "CP"
  DeleteRegValue HKLM "Software\Microsoft\Windows\CurrentVersion\Run" "cp"

  ; 4. Final sweep of install directory
  RMDir /r "$INSTDIR"

  ; 5. Refresh Windows Explorer notification area so zero ghost tray icons linger
  FindWindow $0 "Shell_TrayWnd" ""
  ${If} $0 != 0
    FindWindowEx $1 $0 "TrayNotifyWnd" ""
    ${If} $1 != 0
      FindWindowEx $2 $1 "SysPager" ""
      ${If} $2 != 0
        FindWindowEx $3 $2 "ToolbarWindow32" ""
      ${Else}
        FindWindowEx $3 $1 "ToolbarWindow32" ""
      ${EndIf}
      ${If} $3 != 0
        System::Call "user32::RedrawWindow(p r3, p 0, p 0, i 0x0185)"
      ${EndIf}
    ${EndIf}
  ${EndIf}
  FindWindow $0 "NotifyIconOverflowWindow" ""
  ${If} $0 != 0
    FindWindowEx $1 $0 "ToolbarWindow32" ""
    ${If} $1 != 0
      System::Call "user32::RedrawWindow(p r1, p 0, p 0, i 0x0185)"
    ${EndIf}
  ${EndIf}
!macroend

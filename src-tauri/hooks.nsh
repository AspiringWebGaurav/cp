; ==============================================================================
; CP — NSIS Installer Customization Hooks
; Ensures clean installation, no process locks, and proper desktop shortcuts.
; ==============================================================================

!macro NSIS_HOOK_PREINSTALL
  nsExec::Exec 'taskkill /F /IM cp.exe /T'
  nsExec::Exec 'taskkill /F /IM CP.exe /T'
  Sleep 400
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  nsExec::Exec 'taskkill /F /IM cp.exe /T'
  nsExec::Exec 'taskkill /F /IM CP.exe /T'
  Sleep 400
!macroend

!macro NSIS_HOOK_POSTINSTALL
  CreateShortcut "$DESKTOP\CP.lnk" "$INSTDIR\cp.exe" "--show" "$INSTDIR\cp.exe" 0 "" "" "CP (Copy/Paste Engine)"
  Delete "$INSTDIR\*.old"
  Delete "$INSTDIR\*.staged"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  Delete "$DESKTOP\CP.lnk"
!macroend


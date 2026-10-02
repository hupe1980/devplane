; Hooks into the Windows installer the Tauri bundler writes (wired as
; `bundle.windows.nsis.installerHooks` in tauri.conf.json).
;
; Uninstalling removes the app and nothing of the person's: history and their
; own preferences live in %USERPROFILE%\.devplane, which is left in place and
; named, so nobody wonders whether it went.

!macro NSIS_HOOK_POSTUNINSTALL
  DetailPrint "Your history in $PROFILE\.devplane was left in place."
!macroend

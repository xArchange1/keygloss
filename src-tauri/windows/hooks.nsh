; Included near the top of Tauri's NSIS template, before the finish page is defined.
; Leave the finish page's "Create desktop shortcut" box unchecked.
!define MUI_FINISHPAGE_SHOWREADME_NOTCHECKED

; The uninstaller's "Delete the application data" box: also delete the app's own dirs
; (src-tauri/src/paths.rs). The template only deletes the ones named after the identifier.
!macro KEYGLOSS_DELETE_XDG VAR
  Push $0
  ReadEnvStr $0 ${VAR}
  ${If} $0 != ""
    RmDir /r "$0\keygloss"
  ${EndIf}
  Pop $0
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    SetShellVarContext current
    RmDir /r "$APPDATA\Keygloss"
    RmDir /r "$LOCALAPPDATA\Keygloss"
    !insertmacro KEYGLOSS_DELETE_XDG XDG_CONFIG_HOME
    !insertmacro KEYGLOSS_DELETE_XDG XDG_DATA_HOME
    !insertmacro KEYGLOSS_DELETE_XDG XDG_STATE_HOME
    !insertmacro KEYGLOSS_DELETE_XDG XDG_CACHE_HOME
  ${EndIf}
!macroend

; Installer hooks (bundle > windows > nsis > installerHooks in tauri.conf.json).

; 0.4.3 changed the Windows publisher from "Edgar Mendonca" to "Datum". The installer remembers
; the install folder under Software\<publisher>\<product>, so an update would no longer find a
; folder chosen at the first install: read it from the old key, use it, and remove the old key.
!macro NSIS_HOOK_PREINSTALL
  ReadRegStr $R9 SHCTX "Software\Edgar Mendonca\Datum Plan" ""
  ${If} $R9 != ""
  ${AndIf} $R9 != $INSTDIR
    RMDir $INSTDIR
    StrCpy $INSTDIR $R9
    SetOutPath $INSTDIR
  ${EndIf}
  DeleteRegKey SHCTX "Software\Edgar Mendonca\Datum Plan"
  DeleteRegKey /ifempty SHCTX "Software\Edgar Mendonca"
!macroend

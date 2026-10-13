; Registers the native host for Chrome, Edge and Firefox (HKCU) after the files are in place, and
; removes the registration on uninstall. Inserted into the cargo-packager NSIS template through
; packager.json "preinstallSection" (keep this file and that string in sync); sections run in the
; order they are declared, so the binary is extracted here as well and the main section later
; overwrites it with the same file.
Section "-AegisRegisterNativeHost"
  SetOutPath "$INSTDIR"
  File "${MAINBINARYSRCPATH}"
  DetailPrint "Registering the Aegis Http native host for Chrome, Edge and Firefox..."
  nsExec::ExecToLog '"$INSTDIR\aegis-host.exe" --install'
  Pop $0
  StrCmp $0 "0" aegis_registered
    MessageBox MB_OK|MB_ICONEXCLAMATION "Registering the native host failed (exit code $0). Run $\"$INSTDIR\aegis-host.exe$\" --install manually."
  aegis_registered:
SectionEnd

Section "un.AegisUnregisterNativeHost"
  DeleteRegKey HKCU "Software\Google\Chrome\NativeMessagingHosts\com.aegis.http.gpg"
  DeleteRegKey HKCU "Software\Microsoft\Edge\NativeMessagingHosts\com.aegis.http.gpg"
  DeleteRegKey HKCU "Software\Mozilla\NativeMessagingHosts\com.aegis.http.gpg"
  Delete "$INSTDIR\com.aegis.http.gpg.json"
  Delete "$INSTDIR\com.aegis.http.gpg-firefox.json"
SectionEnd

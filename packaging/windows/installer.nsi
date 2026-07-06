; ITSaNAS Windows installer.
;
; Installs per-user (no admin/UAC prompt required), same approach as
; Dropbox/Google Drive/Slack: everything lands under
; %LOCALAPPDATA%\Programs\ITSaNAS, with a Start Menu entry, a Desktop
; shortcut, and a login autostart entry for the GUI (which launches the
; daemon itself if it isn't already running - see itsanas-gui's
; ensure_daemon_running()). Vault data (%APPDATA%\itsanas) and the synced
; folder (%USERPROFILE%\ITSaNAS) are left alone by the uninstaller, same as
; any real sync client would.

!include "MUI2.nsh"

Name "ITSaNAS"
OutFile "..\..\dist\itsanas-installer.exe"
InstallDir "$LOCALAPPDATA\Programs\ITSaNAS"
RequestExecutionLevel user

!define MUI_ABORTWARNING

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

Section "Install"
    SetOutPath "$INSTDIR"
    File "..\..\target\x86_64-pc-windows-gnu\release\itsanas-daemon.exe"
    File "..\..\target\x86_64-pc-windows-gnu\release\itsanas-gui.exe"

    CreateDirectory "$SMPROGRAMS\ITSaNAS"
    CreateShortcut "$SMPROGRAMS\ITSaNAS\ITSaNAS.lnk" "$INSTDIR\itsanas-gui.exe"
    CreateShortcut "$SMPROGRAMS\ITSaNAS\Uninstall ITSaNAS.lnk" "$INSTDIR\uninstall.exe"
    CreateShortcut "$DESKTOP\ITSaNAS.lnk" "$INSTDIR\itsanas-gui.exe"

    ; Launch at login, same as Dropbox/Google Drive - the whole point of a
    ; synced folder is that it's kept in sync without the user thinking
    ; about it.
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "ITSaNAS" "$INSTDIR\itsanas-gui.exe"

    WriteUninstaller "$INSTDIR\uninstall.exe"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ITSaNAS" "DisplayName" "ITSaNAS"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ITSaNAS" "UninstallString" "$INSTDIR\uninstall.exe"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ITSaNAS" "InstallLocation" "$INSTDIR"
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ITSaNAS" "NoModify" 1
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ITSaNAS" "NoRepair" 1
SectionEnd

Section "Uninstall"
    Delete "$INSTDIR\itsanas-daemon.exe"
    Delete "$INSTDIR\itsanas-gui.exe"
    Delete "$INSTDIR\uninstall.exe"
    RMDir "$INSTDIR"

    Delete "$SMPROGRAMS\ITSaNAS\ITSaNAS.lnk"
    Delete "$SMPROGRAMS\ITSaNAS\Uninstall ITSaNAS.lnk"
    RMDir "$SMPROGRAMS\ITSaNAS"
    Delete "$DESKTOP\ITSaNAS.lnk"

    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "ITSaNAS"
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\ITSaNAS"
SectionEnd

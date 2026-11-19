Unicode true
!include "MUI2.nsh"

; 通过 makensis /D 传入: VERSION / SOURCE_EXE / ICON_FILE / OUT_FILE
!ifndef VERSION
  !define VERSION "0.0.0"
!endif
!ifndef SOURCE_EXE
  !define SOURCE_EXE "purepan.exe"
!endif
!ifndef ICON_FILE
  !define ICON_FILE "..\..\desktop\assets\icons\icon.ico"
!endif
!ifndef OUT_FILE
  !define OUT_FILE "PurePan-setup.exe"
!endif

Name "PurePan"
OutFile "${OUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\PurePan"
InstallDirRegKey HKCU "Software\PurePan" "InstallDir"
RequestExecutionLevel user
SetCompressor /SOLID lzma
SetCompressorDictSize 64
BrandingText "PurePan"

VIProductVersion "${VERSION}.0"
VIAddVersionKey "ProductName" "PurePan"
VIAddVersionKey "FileDescription" "PurePan 安装程序"
VIAddVersionKey "FileVersion" "${VERSION}"
VIAddVersionKey "ProductVersion" "${VERSION}"
VIAddVersionKey "LegalCopyright" "PurePan"

Icon "${ICON_FILE}"
UninstallIcon "${ICON_FILE}"

!define MUI_ICON "${ICON_FILE}"
!define MUI_UNICON "${ICON_FILE}"
!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\purepan.exe"
!define MUI_FINISHPAGE_RUN_TEXT "运行 PurePan"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

Section "PurePan" SecMain
  SetOutPath "$INSTDIR"
  File "/oname=purepan.exe" "${SOURCE_EXE}"

  CreateShortcut "$DESKTOP\PurePan.lnk" "$INSTDIR\purepan.exe" "" "$INSTDIR\purepan.exe" 0
  CreateDirectory "$SMPROGRAMS\PurePan"
  CreateShortcut "$SMPROGRAMS\PurePan\PurePan.lnk" "$INSTDIR\purepan.exe" "" "$INSTDIR\purepan.exe" 0

  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\PurePan" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PurePan" "DisplayName" "PurePan"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PurePan" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PurePan" "DisplayIcon" "$INSTDIR\purepan.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PurePan" "Publisher" "PurePan"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PurePan" "UninstallString" "$INSTDIR\Uninstall.exe"
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PurePan" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PurePan" "NoRepair" 1
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\purepan.exe"
  Delete "$INSTDIR\Uninstall.exe"
  Delete "$DESKTOP\PurePan.lnk"
  Delete "$SMPROGRAMS\PurePan\PurePan.lnk"
  RMDir "$SMPROGRAMS\PurePan"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\PurePan"
  DeleteRegKey HKCU "Software\PurePan"
SectionEnd

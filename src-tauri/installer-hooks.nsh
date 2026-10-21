; Extra steps for Pixie's Windows installer (referenced by bundle.windows.nsis.installerHooks in tauri.conf.json).
;
; Pixie is built with Microsoft's Visual C++ compiler and needs the "Visual C++ 2015-2022 Redistributable (x64)" at run
; time; without it the program will not start. Most computers have it, but a clean one may not. This hook checks for it
; before the files are copied and, when it is missing or too old, downloads Microsoft's installer and runs it (Windows
; shows its administrator prompt, because the runtime installs for all users).

!ifndef VCREDIST_URL
  !define VCREDIST_URL "https://aka.ms/vs/17/release/vc_redist.x64.exe"
!endif
; Pixie is built with MSVC 14.4x. Older 14.x runtimes crash in the C++ standard library, so they count as missing.
!ifndef VCREDIST_MIN_MINOR
  !define VCREDIST_MIN_MINOR 40
!endif

Var PixieVcNeeded

; Sets $PixieVcNeeded to "1" when the computer lacks a recent enough Visual C++ runtime (x64), "0" when it has one.
Function PixieCheckVcRuntime
  Push $0
  Push $1
  Push $2
  SetRegView 64
  ReadRegDWORD $0 HKLM "SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64" "Installed"
  ReadRegDWORD $1 HKLM "SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64" "Major"
  ReadRegDWORD $2 HKLM "SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64" "Minor"
  SetRegView 32
  ${If} $0 == 1
  ${AndIf} $1 == 14
  ${AndIf} $2 >= ${VCREDIST_MIN_MINOR}
    StrCpy $PixieVcNeeded "0"
  ${Else}
    StrCpy $PixieVcNeeded "1"
  ${EndIf}
  Pop $2
  Pop $1
  Pop $0
FunctionEnd

; Downloads and runs Microsoft's installer for the runtime, then checks that it took.
Function PixieInstallVcRuntime
  Push $0
  DetailPrint "Downloading the Microsoft Visual C++ runtime (Pixie needs it to start)..."
  Delete "$TEMP\pixie-vc_redist.x64.exe"
  NSISdl::download "${VCREDIST_URL}" "$TEMP\pixie-vc_redist.x64.exe"
  Pop $0
  ${If} $0 != "success"
    DetailPrint "The runtime could not be downloaded: $0"
    MessageBox MB_ICONEXCLAMATION|MB_OK "Pixie needs the Microsoft Visual C++ runtime, which could not be downloaded ($0).$\r$\n$\r$\nPixie will not start until it is installed. Download and run it from:$\r$\n${VCREDIST_URL}" /SD IDOK
    Goto pixie_vc_done
  ${EndIf}
  DetailPrint "Installing the Microsoft Visual C++ runtime..."
  ; Through the shell, so Windows can show its administrator prompt (the runtime installer requires it).
  ${If} ${Silent}
    ExecShellWait "open" "$TEMP\pixie-vc_redist.x64.exe" "/install /quiet /norestart" SW_HIDE
  ${Else}
    ExecShellWait "open" "$TEMP\pixie-vc_redist.x64.exe" "/install /passive /norestart" SW_SHOWNORMAL
  ${EndIf}
  Delete "$TEMP\pixie-vc_redist.x64.exe"
  Call PixieCheckVcRuntime
  ${If} $PixieVcNeeded == "1"
    DetailPrint "The Microsoft Visual C++ runtime was not installed."
    MessageBox MB_ICONEXCLAMATION|MB_OK "The Microsoft Visual C++ runtime was not installed, so Pixie will not start yet.$\r$\n$\r$\nInstall it from:$\r$\n${VCREDIST_URL}" /SD IDOK
  ${Else}
    DetailPrint "The Microsoft Visual C++ runtime is installed."
  ${EndIf}
  pixie_vc_done:
  Pop $0
FunctionEnd

!macro NSIS_HOOK_PREINSTALL
  Call PixieCheckVcRuntime
  ${If} $PixieVcNeeded == "1"
    Call PixieInstallVcRuntime
  ${EndIf}
!macroend

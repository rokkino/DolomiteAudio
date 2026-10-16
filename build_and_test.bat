@echo off
rem Build DolomiteAudioRT and run the end-to-end ASIO self test (before vs after).
cd /d "%~dp0"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
set CARGO_HTTP_CHECK_REVOKE=false
set LOG=build_and_test.log
echo === build_and_test %DATE% %TIME% === > %LOG%

tasklist /FI "IMAGENAME eq Studio One.exe" | find /I "Studio One" >nul && (
  echo [WARN] Studio One is running: audio_asio.dll is locked, close it first. >> %LOG%
)

if not exist target\release\audio_asio_before.dll (
  if exist target\release\audio_asio.dll copy /Y target\release\audio_asio.dll target\release\audio_asio_before.dll >> %LOG%
)

echo. >> %LOG%
echo === [1/3] cargo build --release === >> %LOG%
cargo --version >> %LOG% 2>&1
cargo build --release -p audio-asio -p audio-control -p audio-bench >> %LOG% 2>&1
if errorlevel 1 (
  echo [INFO] default toolchain failed, retrying with stable-x86_64-pc-windows-gnu >> %LOG%
  cargo +stable-x86_64-pc-windows-gnu build --release -p audio-asio -p audio-control -p audio-bench >> %LOG% 2>&1
  if errorlevel 1 ( echo BUILD FAILED >> %LOG% & echo ALL_DONE >> %LOG% & exit /b 1 )
)
echo BUILD OK >> %LOG%

echo. >> %LOG%
echo === [2/3] unit tests (resampler) === >> %LOG%
cargo test --release -p audio-core stream_resampler >> %LOG% 2>&1

echo. >> %LOG%
echo === [3/3] end-to-end self test (fake DAW + loopback recording) === >> %LOG%
set T=target\release\asio_selftest.exe
if exist target\release\audio_asio_before.dll (
  echo --- BEFORE: old driver --- >> %LOG%
  %T% target\release\audio_asio_before.dll 48000 512 4 >> %LOG% 2>&1
  %T% target\release\audio_asio_before.dll 44100 256 4 >> %LOG% 2>&1
)
echo --- AFTER: new driver --- >> %LOG%
%T% target\release\audio_asio.dll 48000 512 4 >> %LOG% 2>&1
%T% target\release\audio_asio.dll 44100 256 4 >> %LOG% 2>&1
%T% target\release\audio_asio.dll 48000 64 4 >> %LOG% 2>&1
%T% target\release\audio_asio.dll 44100 1024 4 >> %LOG% 2>&1
copy /Y "%APPDATA%\DolomiteAudio\asio_debug.log" asio_debug_new.log >nul 2>&1
echo ALL_DONE >> %LOG%

@echo off
cd /d "%~dp0"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
set LOG=test_v2.log
echo === test_v2 %DATE% %TIME% === > %LOG%
cargo build --release --target-dir target\v2 -p audio-asio >> %LOG% 2>&1 || (echo BUILD FAILED >> %LOG% & echo ALL_DONE >> %LOG% & exit /b 1)
cargo build --release --target-dir target\v2 -p audio-control -p audio-bench >> %LOG% 2>&1 || (echo BUILD FAILED >> %LOG% & echo ALL_DONE >> %LOG% & exit /b 1)
if not exist target\v2\release\audio_asio.dll copy /Y target\v2\release\deps\audio_asio.dll target\v2\release\ >> %LOG%
echo BUILD OK >> %LOG%
set T=target\v2\release\asio_selftest.exe
set D=target\v2\release\audio_asio.dll
%T% --list >> %LOG% 2>&1
echo. >> %LOG%
echo ##### FxSound default device, compatible vs low-latency >> %LOG%
%T% %D% 48000 128 4 0.3 --engine shared >> %LOG% 2>&1
%T% %D% 48000 128 4 0.3 --engine low >> %LOG% 2>&1
echo ##### Realtek direct >> %LOG%
%T% %D% 48000 128 4 0.3 --device realtek --engine shared >> %LOG% 2>&1
%T% %D% 48000 128 4 0.3 --device realtek --engine low >> %LOG% 2>&1
%T% %D% 44100 128 4 0.3 --device realtek --engine low >> %LOG% 2>&1
%T% %D% 48000 64 6 0.6 --device realtek --engine low >> %LOG% 2>&1
echo ##### Realtek exclusive >> %LOG%
%T% %D% 48000 128 4 0.3 --device realtek --engine exclusive >> %LOG% 2>&1
%T% %D% 48000 64 6 0.6 --device realtek --engine exclusive >> %LOG% 2>&1
%T% %D% 44100 64 4 0.3 --device realtek --engine exclusive >> %LOG% 2>&1
%T% %D% 48000 32 4 0.5 --device realtek --engine exclusive >> %LOG% 2>&1
copy /Y "%APPDATA%\DolomiteAudio\asio_debug.log" asio_debug_v2.log >nul 2>&1
echo ALL_DONE >> %LOG%

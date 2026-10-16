@echo off
cd /d "%~dp0"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
set LOG=test_reference.log
echo === test_reference %DATE% %TIME% === > %LOG%
cargo build --release -p audio-bench --bins >> %LOG% 2>&1 || (echo BUILD FAILED >> %LOG% & echo ALL_DONE >> %LOG% & exit /b 1)
set T=target\release\asio_selftest.exe
%T% --reference selftest_reference.wav >> %LOG% 2>&1
%T% target\release\audio_asio_before.dll 48000 512 4 >> %LOG% 2>&1
%T% target\release\audio_asio.dll 48000 512 4 >> %LOG% 2>&1
%T% target\release\audio_asio.dll 44100 256 4 >> %LOG% 2>&1
echo ALL_DONE >> %LOG%

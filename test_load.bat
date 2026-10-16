@echo off
cd /d "%~dp0"
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
set LOG=test_load.log
echo === test_load %DATE% %TIME% === > %LOG%
echo --- kernel service --- >> %LOG%
sc query ZeroLatencyAudio >> %LOG% 2>&1
sc query audio_kernel >> %LOG% 2>&1
cargo build --release -p audio-bench --bins >> %LOG% 2>&1 || (echo BUILD FAILED >> %LOG% & echo ALL_DONE >> %LOG% & exit /b 1)
set T=target\release\asio_selftest.exe
%T% target\release\audio_asio_before.dll 44100 256 4 0 >> %LOG% 2>&1
%T% target\release\audio_asio.dll 44100 256 4 0 >> %LOG% 2>&1
%T% target\release\audio_asio_before.dll 48000 128 4 0.6 >> %LOG% 2>&1
%T% target\release\audio_asio.dll 48000 128 4 0.6 >> %LOG% 2>&1
%T% target\release\audio_asio_before.dll 44100 512 4 0.6 >> %LOG% 2>&1
%T% target\release\audio_asio.dll 44100 512 4 0.6 >> %LOG% 2>&1
echo ALL_DONE >> %LOG%

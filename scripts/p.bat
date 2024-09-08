@echo off
:loop
@echo %1
shift
if not "%~1"=="" goto loop
pause

@echo off
echo yes > D:\bat_works.txt
echo Current dir: %cd% >> D:\bat_works.txt

:Loop
if "%1"=="" goto completed 
for %%f in (%1) do echo %%f  >> D:\bat_works.txt
shift 
goto Loop 
:completed
echo All args printed >> D:\bat_works.txt
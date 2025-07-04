#!/bin/bash

cd examples_srcs/dotnet_program
dotnet-bin-6.0 build -r win-x64 --configuration Release --self-contained

cd ../rust_program/
cargo xwin build --target x86_64-pc-windows-msvc --release

cd ../..

cp examples_srcs/dotnet_program/bin/Release/net6.0/win-x64/dotnet_program.dll examples/example.dll
cp examples_srcs/rust_program/target/x86_64-pc-windows-msvc/release/rust_program.exe examples/example.exe
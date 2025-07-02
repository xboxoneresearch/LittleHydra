Write-Host "PS1 execution works" > D:\ps1_works.txt
Write-Host "Current dir: $(Get-Location)" >> D:\ps1_works.txt

if ($args.Count -eq 0) {
    Write-Host "No args passed" >> D:\ps1_works.txt
} else {
    for ($i = 0; $i -lt $args.Count; $i++) {
        Write-Host "Arg $i: $($args[$i])" >> D:\ps1_works.txt
    }
}

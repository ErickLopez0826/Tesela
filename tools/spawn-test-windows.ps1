# Abre N ventanas de prueba (WPF) con título "<Prefix> <i>".
# Solo estas ventanas son las que las pruebas del gestor mueven (ver DECISIONS.md D-1).
#
# Importante: las pruebas copian powershell.exe como "Lively.exe" para ejecutar este script.
# La config de GlazeWM ignora (regla 'ignore') las ventanas del proceso Lively con clase
# HwndWrapper, que es la de las ventanas WPF. Así GlazeWM, que sigue corriendo como respaldo,
# no pelea con la app por estas ventanas y no hace falta tocar su config.
#
# Imprime "HANDLES h1 h2 ..." y termina cuando se cierran todas las ventanas.
param(
    [int]$Count = 3,
    [string]$Prefix = "WMTEST"
)
Add-Type -AssemblyName PresentationFramework
Add-Type -AssemblyName WindowsBase

$app = New-Object System.Windows.Application
$handles = @()
for ($i = 1; $i -le $Count; $i++) {
    $w = New-Object System.Windows.Window
    $w.Title = "$Prefix $i"
    $w.WindowStartupLocation = 'Manual'
    $w.Left = 80 + 30 * $i
    $w.Top = 80 + 30 * $i
    $w.Width = 420
    $w.Height = 300
    $w.Show()
    $handles += (New-Object System.Windows.Interop.WindowInteropHelper $w).Handle.ToInt64()
}
[Console]::Out.WriteLine("HANDLES " + ($handles -join ' '))
[Console]::Out.Flush()
$app.Run() | Out-Null

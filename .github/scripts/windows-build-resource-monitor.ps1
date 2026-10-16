[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)]
  [ValidateSet('Monitor', 'Summarize')]
  [string]$Mode,

  [string]$CsvPath,
  [string]$StopPath,
  [string]$ReadyPath,
  [string]$TargetPath,
  [string]$SummaryPath,
  [string]$StepSummaryPath,
  [string]$BuildOutcome,
  [ValidateRange(0, 10800)]
  [int]$BuildDurationSeconds = 0,
  [ValidateRange(5, 30)]
  [int]$IntervalSeconds = 5,
  [ValidateRange(60, 10800)]
  [int]$MaxRuntimeSeconds = 10800
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-PathArgument {
  param(
    [Parameter(Mandatory = $true)][string]$Name,
    [AllowEmptyString()][string]$Value
  )

  if ([string]::IsNullOrWhiteSpace($Value)) {
    throw "$Name 未设置。"
  }
}

function Get-SystemCpuPercent {
  try {
    $counter = Get-CimInstance `
      -ClassName Win32_PerfFormattedData_PerfOS_Processor `
      -Filter "Name='_Total'" -ErrorAction Stop |
      Select-Object -First 1
    if ($null -eq $counter) {
      throw '未找到 _Total CPU 计数器。'
    }
    return [double]$counter.PercentProcessorTime
  } catch {
    $processors = @(Get-CimInstance -ClassName Win32_Processor `
      -Property LoadPercentage -ErrorAction Stop)
    if ($processors.Count -eq 0) {
      throw
    }
    return [double](($processors |
      Measure-Object -Property LoadPercentage -Average).Average)
  }
}

function Invoke-Monitor {
  Assert-PathArgument -Name 'CsvPath' -Value $CsvPath
  Assert-PathArgument -Name 'StopPath' -Value $StopPath
  Assert-PathArgument -Name 'ReadyPath' -Value $ReadyPath
  Assert-PathArgument -Name 'TargetPath' -Value $TargetPath

  $csvDirectory = [IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($CsvPath))
  New-Item -ItemType Directory -Path $csvDirectory -Force | Out-Null

  $logicalCpu = [Environment]::ProcessorCount
  $previousCpu = @{}
  $previousAt = $null
  $stopwatch = [Diagnostics.Stopwatch]::StartNew()
  $sampleNumber = 0
  $writer = $null
  $toolNames = [Collections.Generic.HashSet[string]]::new(
    [StringComparer]::OrdinalIgnoreCase
  )
  @(
    'cargo', 'rustc', 'cc', 'gcc', 'g++', 'ld', 'lld', 'rust-lld',
    'collect2', 'cc1', 'cc1plus', 'ar', 'cmake', 'ninja', 'make'
  ) | ForEach-Object { [void]$toolNames.Add($_) }

  $targetRoot = [IO.Path]::GetPathRoot([IO.Path]::GetFullPath($TargetPath))
  if ([string]::IsNullOrWhiteSpace($targetRoot)) {
    throw "无法解析目标盘：$TargetPath"
  }

  $columns = @(
    'Sample', 'TimestampUtc', 'ElapsedSeconds', 'SystemCpuPercent',
    'LogicalCpuCount', 'MemoryAvailableBytes', 'MemoryTotalBytes',
    'MemoryUsedPercent', 'ToolProcessCount', 'ToolProcessNames',
    'ToolWorkingSetBytes', 'ToolCpuSeconds', 'ToolCpuPercent',
    'TargetDrive', 'TargetFreeBytes', 'TargetTotalBytes',
    'TargetFreePercent', 'MonitorError'
  )

  try {
    $writer = [IO.StreamWriter]::new(
      $CsvPath, $false, [Text.UTF8Encoding]::new($false)
    )
    $writer.AutoFlush = $true
    $writer.WriteLine(($columns -join ','))
    "resource monitor pid=$PID target=$targetRoot interval=$IntervalSeconds"

    while ($true) {
      if (Test-Path -LiteralPath $StopPath) {
        break
      }
      if ($stopwatch.Elapsed.TotalSeconds -ge $MaxRuntimeSeconds) {
        [Console]::Error.WriteLine('resource monitor reached max runtime')
        break
      }

      $sampleNumber++
      $now = [DateTime]::UtcNow
      $errors = [Collections.Generic.List[string]]::new()
      $systemCpu = $null
      try {
        $systemCpu = Get-SystemCpuPercent
      } catch {
        [void]$errors.Add("cpu:$($_.Exception.Message)")
      }

      $memoryAvailable = $null
      $memoryTotal = $null
      try {
        $os = Get-CimInstance -ClassName Win32_OperatingSystem `
          -Property TotalVisibleMemorySize, FreePhysicalMemory `
          -ErrorAction Stop | Select-Object -First 1
        if ($null -eq $os) {
          throw '未找到操作系统内存信息。'
        }
        $memoryTotal = [double]$os.TotalVisibleMemorySize * 1KB
        $memoryAvailable = [double]$os.FreePhysicalMemory * 1KB
      } catch {
        [void]$errors.Add("memory:$($_.Exception.Message)")
      }

      $current = @{}
      foreach ($name in $toolNames) {
        foreach ($process in @(Get-Process -Name $name `
          -ErrorAction SilentlyContinue)) {
          try {
            $processId = [int]$process.Id
            if ($current.ContainsKey($processId)) {
              continue
            }
            $current[$processId] = [pscustomobject]@{
              Pid = $processId
              Name = [string]$process.ProcessName
              WorkingSetBytes = [int64]$process.WorkingSet64
              CpuSeconds = [double]$process.TotalProcessorTime.TotalSeconds
            }
          } catch {
            [void]$errors.Add("proc[$name]:$($_.Exception.Message)")
          }
        }
      }

      $tools = @($current.Values)
      $toolWorkingSet = 0L
      $toolCpuSeconds = 0.0
      foreach ($tool in $tools) {
        $toolWorkingSet += $tool.WorkingSetBytes
        $toolCpuSeconds += $tool.CpuSeconds
      }

      $toolCpuPercent = $null
      if ($null -ne $previousAt) {
        $elapsed = ($now - $previousAt).TotalSeconds
        if ($elapsed -gt 0 -and $logicalCpu -gt 0) {
          $cpuDelta = 0.0
          foreach ($tool in $tools) {
            if ($previousCpu.ContainsKey($tool.Pid)) {
              $delta = $tool.CpuSeconds - [double]$previousCpu[$tool.Pid]
              if ($delta -ge 0) {
                $cpuDelta += $delta
              }
            }
          }
          $toolCpuPercent = 100.0 * $cpuDelta / ($elapsed * $logicalCpu)
        }
      }
      $previousCpu = @{}
      foreach ($tool in $tools) {
        $previousCpu[$tool.Pid] = $tool.CpuSeconds
      }
      $previousAt = $now

      $targetFree = $null
      $targetTotal = $null
      try {
        $drive = [IO.DriveInfo]::new($targetRoot)
        if (!$drive.IsReady) {
          throw "目标盘未就绪：$targetRoot"
        }
        $targetFree = [double]$drive.AvailableFreeSpace
        $targetTotal = [double]$drive.TotalSize
      } catch {
        [void]$errors.Add("disk:$($_.Exception.Message)")
      }

      $memoryUsedPercent = if (
        $null -ne $memoryAvailable -and $null -ne $memoryTotal -and
        $memoryTotal -gt 0
      ) {
        100.0 * (1.0 - $memoryAvailable / $memoryTotal)
      } else {
        $null
      }
      $targetFreePercent = if (
        $null -ne $targetFree -and $null -ne $targetTotal -and
        $targetTotal -gt 0
      ) {
        100.0 * $targetFree / $targetTotal
      } else {
        $null
      }
      $errorText = (($errors | ForEach-Object {
        $_ -replace '[\r\n]+', ' '
      }) -join ' | ')

      $row = [pscustomobject][ordered]@{
        Sample = $sampleNumber
        TimestampUtc = $now.ToString('o')
        ElapsedSeconds = [math]::Round($stopwatch.Elapsed.TotalSeconds, 3)
        SystemCpuPercent = if ($null -eq $systemCpu) {
          $null
        } else {
          [math]::Round($systemCpu, 2)
        }
        LogicalCpuCount = $logicalCpu
        MemoryAvailableBytes = $memoryAvailable
        MemoryTotalBytes = $memoryTotal
        MemoryUsedPercent = if ($null -eq $memoryUsedPercent) {
          $null
        } else {
          [math]::Round($memoryUsedPercent, 2)
        }
        ToolProcessCount = $tools.Count
        ToolProcessNames = (@($tools | ForEach-Object { $_.Name } |
          Sort-Object -Unique) -join ';')
        ToolWorkingSetBytes = $toolWorkingSet
        ToolCpuSeconds = [math]::Round($toolCpuSeconds, 3)
        ToolCpuPercent = if ($null -eq $toolCpuPercent) {
          $null
        } else {
          [math]::Round($toolCpuPercent, 2)
        }
        TargetDrive = $targetRoot
        TargetFreeBytes = $targetFree
        TargetTotalBytes = $targetTotal
        TargetFreePercent = if ($null -eq $targetFreePercent) {
          $null
        } else {
          [math]::Round($targetFreePercent, 2)
        }
        MonitorError = $errorText
      }
      foreach ($line in ($row | ConvertTo-Csv -NoTypeInformation |
        Select-Object -Skip 1)) {
        $writer.WriteLine($line)
      }

      if ($sampleNumber -eq 1) {
        New-Item -ItemType File -Path $ReadyPath -Force | Out-Null
      }
      if (Test-Path -LiteralPath $StopPath) {
        break
      }
      Start-Sleep -Seconds $IntervalSeconds
    }
  } finally {
    if ($null -ne $writer) {
      try {
        $writer.Flush()
        $writer.Dispose()
      } catch {}
    }
  }
}

function Convert-ToNumber {
  param([AllowEmptyString()][string]$Text)

  if ([string]::IsNullOrWhiteSpace($Text)) {
    return $null
  }
  $styles = [Globalization.NumberStyles]::Float -bor
    [Globalization.NumberStyles]::AllowThousands
  $value = 0.0
  if ([double]::TryParse(
    $Text, $styles, [Globalization.CultureInfo]::InvariantCulture,
    [ref]$value
  )) {
    return $value
  }
  if ([double]::TryParse(
    $Text, $styles, [Globalization.CultureInfo]::CurrentCulture,
    [ref]$value
  )) {
    return $value
  }
  return $null
}

function Get-Statistic {
  param(
    [Parameter(Mandatory = $true)]
    [AllowEmptyCollection()]
    [object[]]$Rows,
    [Parameter(Mandatory = $true)][string]$Name,
    [Parameter(Mandatory = $true)]
    [ValidateSet('Average', 'Maximum', 'Minimum')]
    [string]$Operation
  )

  $values = @($Rows | ForEach-Object {
    $number = Convert-ToNumber -Text ([string]$_.$Name)
    if ($null -ne $number) {
      $number
    }
  })
  if ($values.Count -eq 0) {
    return $null
  }
  $result = switch ($Operation) {
    'Average' { ($values | Measure-Object -Average).Average }
    'Maximum' { ($values | Measure-Object -Maximum).Maximum }
    'Minimum' { ($values | Measure-Object -Minimum).Minimum }
  }
  return $result
}

function Format-Number {
  param($Value)

  if ($null -eq $Value) {
    return 'n/a'
  }
  return ([double]$Value).ToString(
    'F2', [Globalization.CultureInfo]::InvariantCulture
  )
}

function Format-GiB {
  param($Value)

  if ($null -eq $Value) {
    return 'n/a'
  }
  return (([double]$Value) / 1GB).ToString(
    'F2', [Globalization.CultureInfo]::InvariantCulture
  )
}

function Write-Summary {
  Assert-PathArgument -Name 'CsvPath' -Value $CsvPath
  Assert-PathArgument -Name 'SummaryPath' -Value $SummaryPath

  $rows = @()
  if (Test-Path -LiteralPath $CsvPath) {
    $rows = @(Import-Csv -LiteralPath $CsvPath)
  }

  $lastElapsedSeconds = Get-Statistic `
    -Rows $rows -Name 'ElapsedSeconds' -Operation Maximum
  $coveragePercent = if (
    $BuildDurationSeconds -gt 0 -and $null -ne $lastElapsedSeconds
  ) {
    [math]::Min(100.0, 100.0 * $lastElapsedSeconds / $BuildDurationSeconds)
  } else {
    $null
  }

  $summary = [ordered]@{
    BuildOutcome = $BuildOutcome
    BuildDurationSeconds = $BuildDurationSeconds
    SampleIntervalSeconds = $IntervalSeconds
    Samples = $rows.Count
    LastElapsedSeconds = $lastElapsedSeconds
    CoveragePercent = $coveragePercent
    SystemCpuAveragePercent = Get-Statistic `
      -Rows $rows -Name 'SystemCpuPercent' -Operation Average
    SystemCpuPeakPercent = Get-Statistic `
      -Rows $rows -Name 'SystemCpuPercent' -Operation Maximum
    MemoryUsedPeakPercent = Get-Statistic `
      -Rows $rows -Name 'MemoryUsedPercent' -Operation Maximum
    MemoryAvailableMinBytes = Get-Statistic `
      -Rows $rows -Name 'MemoryAvailableBytes' -Operation Minimum
    MemoryTotalBytes = Get-Statistic `
      -Rows $rows -Name 'MemoryTotalBytes' -Operation Maximum
    ToolProcessCountPeak = Get-Statistic `
      -Rows $rows -Name 'ToolProcessCount' -Operation Maximum
    ToolWorkingSetPeakBytes = Get-Statistic `
      -Rows $rows -Name 'ToolWorkingSetBytes' -Operation Maximum
    ToolCpuAveragePercent = Get-Statistic `
      -Rows $rows -Name 'ToolCpuPercent' -Operation Average
    ToolCpuPeakPercent = Get-Statistic `
      -Rows $rows -Name 'ToolCpuPercent' -Operation Maximum
    TargetFreeMinBytes = Get-Statistic `
      -Rows $rows -Name 'TargetFreeBytes' -Operation Minimum
    TargetTotalBytes = Get-Statistic `
      -Rows $rows -Name 'TargetTotalBytes' -Operation Maximum
    TargetFreePercentMin = Get-Statistic `
      -Rows $rows -Name 'TargetFreePercent' -Operation Minimum
    MonitorErrorRows = @($rows | Where-Object {
      ![string]::IsNullOrWhiteSpace($_.MonitorError)
    }).Count
  }

  $summaryDirectory = [IO.Path]::GetDirectoryName(
    [IO.Path]::GetFullPath($SummaryPath)
  )
  New-Item -ItemType Directory -Path $summaryDirectory -Force | Out-Null
  $summary | ConvertTo-Json -Depth 4 |
    Set-Content -LiteralPath $SummaryPath -Encoding utf8

  if (![string]::IsNullOrWhiteSpace($StepSummaryPath)) {
    @"
## Windows GNU 构建资源监控

- 构建结果：``$($summary.BuildOutcome)``
- 构建耗时：``$($summary.BuildDurationSeconds)`` 秒
- 采样：``$($summary.Samples)`` 条，每 $IntervalSeconds 秒一次，覆盖 $(Format-Number $summary.CoveragePercent)%
- 全机 CPU：平均 $(Format-Number $summary.SystemCpuAveragePercent)% / 峰值 $(Format-Number $summary.SystemCpuPeakPercent)%
- 内存：峰值使用 $(Format-Number $summary.MemoryUsedPeakPercent)% / 最低可用 $(Format-GiB $summary.MemoryAvailableMinBytes) GiB / 总计 $(Format-GiB $summary.MemoryTotalBytes) GiB
- 构建工具进程：峰值 $(Format-Number $summary.ToolProcessCountPeak) 个 / 工作集峰值 $(Format-GiB $summary.ToolWorkingSetPeakBytes) GiB
- 构建工具 CPU：平均 $(Format-Number $summary.ToolCpuAveragePercent)% / 峰值 $(Format-Number $summary.ToolCpuPeakPercent)%（按全部逻辑 CPU 归一化）
- 目标盘：最低可用 $(Format-GiB $summary.TargetFreeMinBytes) GiB / 总计 $(Format-GiB $summary.TargetTotalBytes) GiB
- 监控异常样本：``$($summary.MonitorErrorRows)``
"@ | Add-Content -LiteralPath $StepSummaryPath -Encoding utf8
  }

  if ($BuildOutcome -eq 'success') {
    if ($BuildDurationSeconds -ge (2 * $IntervalSeconds) -and $rows.Count -lt 2) {
      throw "资源监控样本不足：$($rows.Count) 条。"
    }
    if (
      $BuildDurationSeconds -ge 60 -and
      ($null -eq $coveragePercent -or $coveragePercent -lt 80.0)
    ) {
      throw "资源监控覆盖不足：$(Format-Number $coveragePercent)%。"
    }
    $toolPeak = $summary.ToolProcessCountPeak
    if (
      $BuildDurationSeconds -ge 60 -and
      ($null -eq $toolPeak -or [double]$toolPeak -lt 1.0)
    ) {
      throw '资源监控未捕获任何 Cargo/Rust/原生构建进程。'
    }
    if (
      $BuildDurationSeconds -ge 60 -and
      (
        $null -eq $summary.SystemCpuAveragePercent -or
        $null -eq $summary.MemoryUsedPeakPercent -or
        $null -eq $summary.TargetFreeMinBytes
      )
    ) {
      throw '资源监控缺少 CPU、内存或目标盘的有效样本。'
    }
  }
}

if ($Mode -eq 'Monitor') {
  Invoke-Monitor
} else {
  Write-Summary
}

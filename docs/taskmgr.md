<div align="center">

<img src="../data/icons/io.github.hafij_deu.Harbor.TaskManager.svg" width="88" alt="Harbor Task Manager icon">

# Harbor Task Manager

**A Windows 11-style Task Manager for GNOME.**

[![Release](https://img.shields.io/github/v/release/Hafij-DEU/harbor?filter=taskmgr-v*&label=release&color=1e88e5)](https://github.com/Hafij-DEU/harbor/releases?q=taskmgr-v&expanded=true)
![Python](https://img.shields.io/badge/Python-GTK%204-3776ab?logo=python&logoColor=white)

[← Harbor](../README.md) · [Files](files.md) · [Settings](../settings/README.md) · [Android](../android/README.md)

</div>

---

![Processes](taskmgr-processes.png)

## Pages

| Page | What it does |
| --- | --- |
| **Processes** | Apps, Background processes and System processes with live CPU, memory, disk and network columns. End task, Efficiency mode, Run new task and Properties. |
| [**Performance**](#performance) | 60-second graphs for CPU (overall or per core), Memory (with compressed zram memory), Swap, every disk, every network adapter and the GPU. |
| [**Memory Sweep**](#memory-sweep) | What fills your RAM, suggestions to free it, and the files held in `/tmp` and `/dev/shm`. |
| **App history** | Resource use per app over time. |
| **Startup apps** | Apps that start when you sign in. |
| **Users** | Resource use per signed-in user. |
| **Details** | Every process; End process tree and Set priority. |
| **Services** | Start, stop and restart system services. |
| [**Settings**](#settings) | Update speed, start page, high-memory warning and memory optimisation. |

Linux has no per-process network speed without root. The Network column shows open connections per process; the total at the top is real.

## Performance

| CPU | Memory |
| --- | --- |
| ![CPU](taskmgr-performance-cpu.png) | ![Memory](taskmgr-performance-memory.png) |
| **Per-core CPU** | **Disk** |
| ![Cores](taskmgr-performance-cores.png) | ![Disk](taskmgr-performance-disk.png) |
| **Network** | **GPU** |
| ![Network](taskmgr-performance-network.png) | ![GPU](taskmgr-performance-gpu-light.png) |

## Memory Sweep

![Memory Sweep](taskmgr-sweep.png)

## Settings

Memory optimisation sets zram size and compression, the swap file, zswap, swappiness and cache pressure. Changes that need root ask for your password through polkit.

![Settings](taskmgr-settings.png)

## Gallery

<table>
<tr>
<td width="33%"><img src="taskmgr-processes-light.png" alt=""><br><sub>Processes, light</sub></td>
<td width="33%"><img src="taskmgr-history.png" alt=""><br><sub>App history</sub></td>
<td width="33%"><img src="taskmgr-startup.png" alt=""><br><sub>Startup apps</sub></td>
</tr>
<tr>
<td><img src="taskmgr-users.png" alt=""><br><sub>Users</sub></td>
<td><img src="taskmgr-details.png" alt=""><br><sub>Details</sub></td>
<td><img src="taskmgr-services.png" alt=""><br><sub>Services</sub></td>
</tr>
<tr>
<td><img src="taskmgr-performance-memory-details.png" alt=""><br><sub>Memory details</sub></td>
<td><img src="taskmgr-performance-memory-light.png" alt=""><br><sub>Memory, light</sub></td>
<td><img src="taskmgr-performance-swap.png" alt=""><br><sub>Swap</sub></td>
</tr>
<tr>
<td><img src="taskmgr-sweep-light.png" alt=""><br><sub>Memory Sweep, light</sub></td>
<td><img src="taskmgr-settings-light.png" alt=""><br><sub>Settings, light</sub></td>
<td></td>
</tr>
</table>

## Install

```bash
sudo apt install ./harbor-taskmgr_*_all.deb
harbor-taskmgr                          # or: harbor-taskmgr --page performance:memory
```

Get the `.deb` from [Releases](https://github.com/Hafij-DEU/harbor/releases?q=taskmgr-v&expanded=true).

---

<div align="center"><sub><a href="../README.md">Harbor</a> · <a href="files.md">Files</a> · <a href="../settings/README.md">Settings</a> · <a href="../android/README.md">Android</a></sub></div>

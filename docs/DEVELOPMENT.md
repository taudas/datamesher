# Local development environment

Most devs won't have real RDMA-capable NICs (RoCE/InfiniBand hardware) lying around. This sets up a working RDMA device on plain Ethernet using **soft-RoCE** (the `rxe` kernel module), plus notes for WSL2 on Windows.

## Option A: native Linux with soft-RoCE

```bash
sudo apt install -y libibverbs-dev librdmacm-dev rdma-core clang pkg-config build-essential
sudo modprobe rdma_rxe

# bind rxe to an existing network interface, e.g. eth0
sudo rdma link add rxe0 type rxe netdev eth0

# verify a device shows up
ibv_devices
ibv_devinfo
```

`ibv_devices` should list `rxe0`. That's enough for `RdmaEndpoint::open_first_device` to succeed and for QP/MR bring-up to run — soft-RoCE pushes RDMA verbs over normal IP, no special hardware needed. Throughput is bad (it's software), but the API surface is real.

## Option B: WSL2 (Windows host)

WSL2 runs a real Linux kernel, but the default kernel doesn't ship `rdma_rxe`. Two paths:

1. **Custom WSL2 kernel with RDMA modules** — build a WSL2 kernel with `CONFIG_RDMA_RXE` enabled ([microsoft/WSL2-Linux-Kernel](https://github.com/microsoft/WSL2-Linux-Kernel)), swap it in via `.wslconfig`, then follow Option A inside WSL2.
2. **Use a Linux VM instead** — if fighting the WSL2 kernel isn't worth it, a plain Linux VM (Hyper-V, VirtualBox, or a cloud box) with Option A is simpler and gets you the same soft-RoCE setup.

Either way: this repo's crates (`rdma`, `producer`, `consumer`) build and run inside the Linux environment, not on the Windows host directly — `rdma-sys` links against `libibverbs`, which doesn't exist on Windows.

## Sanity check before running the workspace

```bash
ibv_devices          # should list at least one device (rxe0, or real hardware)
ibv_devinfo -d rxe0   # port state should reach ACTIVE once both ends are up
```

If `ibv_devices` is empty, `RdmaEndpoint::open_first_device` returns
`NotFound` immediately — that's the first thing to check when `producer`
or `consumer` fails to start.

## Two-node testing

Producer and consumer are meant to run on separate hosts. For local dev, two soft-RoCE devices on the same machine (or two VMs/WSL2 instances bridged on the same network) work fine — the RC queue pair doesn't care that the "network" is virtual.

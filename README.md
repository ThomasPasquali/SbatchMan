# SbatchMan
A utility to create, launch, and monitor code experiments on SLURM, PBS, or local machines.

Currently in development, check the [design document](design%20document.md) for details.

### Development Guide
Clone the repository and install dependencies:
```bash
# Clone the repository
git clone https://github.com/ThomasPasquali/SbatchMan
cd SbatchMan

# Install dependencies
cargo build

# Run the project
cargo run
```

Simple example:
```bash
cargo run add-cluster --cluster-name my_cluster_name --scheduler Slurm

cargo run list-clusters
```
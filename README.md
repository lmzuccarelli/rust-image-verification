## Overview


This is a simple POC that verifies blobs on disk from a manifest

## POC 


I used a simple approach - Occam's razor

- A scientific and philosophical rule that entities should not be multiplied unnecessarily (KISS)
- Worked with a v2 images for the POC
- only redhat release images have been included for now

## Usage

Clone this repo


```bash

make build 

# execute 
./target/release/rust-image-verification <folder-to-working-dir/ocp-release/4.xx.x-arch/release/> 
```

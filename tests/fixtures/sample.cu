#include <cuda_runtime.h>
#include <stdint.h>

#define BLOCK_SIZE 256

/// Holds per-block reduction state.
struct ReduceState {
    float sum;
    int count;
};

__constant__ float kEpsilon;

__device__ float square(float x) {
    return x * x;
}

__global__ void reduce_kernel(const float *input, float *output, int n) {
    __shared__ float cache[BLOCK_SIZE];
    int tid = threadIdx.x + blockIdx.x * blockDim.x;
    float val = tid < n ? square(input[tid]) : 0.0f;
    cache[threadIdx.x] = val;
    __syncthreads();
    if (threadIdx.x == 0) {
        float total = 0.0f;
        for (int i = 0; i < BLOCK_SIZE; i++) {
            total += cache[i];
        }
        output[blockIdx.x] = total;
    }
}

extern "C" void launch_reduce(const float *input, float *output, int n) {
    int blocks = (n + BLOCK_SIZE - 1) / BLOCK_SIZE;
    reduce_kernel<<<blocks, BLOCK_SIZE>>>(input, output, n);
}

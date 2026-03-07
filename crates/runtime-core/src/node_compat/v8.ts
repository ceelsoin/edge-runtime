function cachedDataVersionTag(): number {
  return 0;
}

function getHeapStatistics() {
  return {
    total_heap_size: 0,
    total_heap_size_executable: 0,
    total_physical_size: 0,
    total_available_size: 0,
    used_heap_size: 0,
    heap_size_limit: 0,
  };
}

const v8Module = { cachedDataVersionTag, getHeapStatistics };

export { cachedDataVersionTag, getHeapStatistics };
export default v8Module;

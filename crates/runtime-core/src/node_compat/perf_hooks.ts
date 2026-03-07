const performanceApi = globalThis.performance;
const PerformanceObserverCtor = globalThis.PerformanceObserver;
const PerformanceEntryCtor = globalThis.PerformanceEntry;

function monitorEventLoopDelay() {
  return {
    enable() {
      return this;
    },
    disable() {
      return this;
    },
    percentile() {
      return 0;
    },
    mean: 0,
    max: 0,
    min: 0,
    stddev: 0,
  };
}

const constants = {
  NODE_PERFORMANCE_GC_MAJOR: 1,
  NODE_PERFORMANCE_GC_MINOR: 2,
  NODE_PERFORMANCE_GC_INCREMENTAL: 4,
  NODE_PERFORMANCE_GC_WEAKCB: 8,
};

const perfHooks = {
  performance: performanceApi,
  PerformanceObserver: PerformanceObserverCtor,
  PerformanceEntry: PerformanceEntryCtor,
  monitorEventLoopDelay,
  constants,
};

export const performance = performanceApi;
export const PerformanceObserver = PerformanceObserverCtor;
export const PerformanceEntry = PerformanceEntryCtor;
export { monitorEventLoopDelay, constants };
export default perfHooks;

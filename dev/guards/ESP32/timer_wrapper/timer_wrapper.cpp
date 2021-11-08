#include "timer_wrapper.h"

namespace {
    static const timer_group_t TIMER_GROUP =  TIMER_GROUP_0;
    static const timer_idx_t TIMER_INDEX = TIMER_0;
    static const u32 TIMER_DIVIDER = getCpuFrequencyMhz() * 1000;

    static TimerWrapper timer;
    static bool isTimerInitialized;
}


TimerWrapper::TimerWrapper() {
    timer_config_t config = {
        .divider = TIMER_DIVIDER,
        .counter_dir = TIMER_COUNT_UP,
        .counter_en = TIMER_START,
        .alarm_en = TIMER_ALARM_DIS,
    };

    timer_init(TIMER_GROUP, TIMER_INDEX, &config);
}

u64 TimerWrapper::getTimestamp() {
    u64 value;
    timer_get_counter_value(TIMER_GROUP, TIMER_INDEX, &value);
    return value;
}

bool TimerWrapper::isTimeElapsed(u64 timestamp, u64 toElapse) {
    return true;
}

TimerWrapper& getTimerInstance() {
    if (!isTimerInitialized)
        timer = TimerWrapper();

    return timer;
}
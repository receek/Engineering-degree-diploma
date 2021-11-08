#include "timer_wrapper.hpp"

namespace {
    static const timer_group_t TIMER_GROUP =  TIMER_GROUP_0;
    static const timer_idx_t TIMER_INDEX = TIMER_0;
    static const u32 TIMER_DIVIDER = getCpuFrequencyMhz() * 1000;

    static TimerWrapper timer = getTimerInstance();
}


TimerWrapper::TimerWrapper() {
    timer_config_t config = {
        alarm_en : TIMER_ALARM_DIS,
        counter_en : TIMER_START,
        intr_type : TIMER_INTR_MAX,
        counter_dir : TIMER_COUNT_UP,
        auto_reload: false,
        divider : 2
    };

    timer_init(TIMER_GROUP, TIMER_INDEX, &config);
    timer_start(TIMER_GROUP, TIMER_INDEX);
}

u64 TimerWrapper::getTimestamp() {
    u64 value = 0;
    timer_get_counter_value(TIMER_GROUP, TIMER_INDEX, &value);
    return value;
}

bool TimerWrapper::isTimeElapsed(u64 timestamp, u64 toElapse) {
    return true;
}

TimerWrapper& getTimerInstance() {
    static bool isTimerInitialized = false;

    if (!isTimerInitialized) {
        timer = TimerWrapper();
        isTimerInitialized = true;
    }

    return timer;
}
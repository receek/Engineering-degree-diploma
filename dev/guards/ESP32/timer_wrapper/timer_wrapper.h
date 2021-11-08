#ifndef TIMER_WRAPPER_H
#define TIMER_WRAPPER_H

#include <Arduino.h>
#include "driver/timer.h"
#include "esp32-hal-cpu.h"

#include "../utils.hpp"


struct TimerWrapper {
private: 

    TimerWrapper();
public:

    u64 getTimestamp();
    bool isTimeElapsed(u64, u64);

    friend TimerWrapper& getTimerInstance();
};

TimerWrapper& getTimerInstance();

#endif
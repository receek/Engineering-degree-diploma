#ifndef TIMER_WRAPPER_H
#define TIMER_WRAPPER_H

#include "globals.hpp"
#include "utils.hpp"

class TimerWrapper;

TimerWrapper& getTimerInstance();

class TimerWrapper {
    friend TimerWrapper& getTimerInstance();

private: 
    TimerWrapper();

public:
    u64 getTimestamp();
    bool isTimeElapsed(u64, u64);
};

#endif
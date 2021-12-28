
#include "miner.hpp"

static const char * stateNames[] = {
    "NotDefined",
    "PoweredOff",
    "Starting",
    "Running",
    "Stopping",
    "HardStopping",
    "Restarting",
    "HardRestarting",
    "Aborted",
    "Unreachable"
};

const char * getStateName(State state) {
    return stateNames[static_cast<u32>(state)];
}

static const char * commandNames[] = {
    "NotDefined",
    "Idle",
    "PowerOn",
    "PowerOff",
    "HardStop",
    "Reset",
    "HardReset",
    "LedReport"
};

const char * getCommandName(Command command) {
    return commandNames[static_cast<u32>(command)];
}

Command getCommandFromName(String& state) {
    for (u32 i = 1; i < (sizeof(commandNames) / sizeof(commandNames[0])); ++i) {
        if (state == commandNames[i])
            return static_cast<Command>(i);
    }
    return Command::NotDefined;
}

TimerWrapper& Miner::timer = getTimerInstance();
MQTTClient *Miner::client = 0;

void Miner::setConfiguration(u8 pinSet_, String & id_) {
    pinSet = pinSet_;
    pinPower = PINOUTS_SET[pinSet][0];
    pinReset = PINOUTS_SET[pinSet][1];
    pinLed = PINOUTS_SET[pinSet][2];

    id = id_;
    commandPrefixTopic = GUARD_PREFIX_TOPIC + "miners/" + id + "/";
    errorLogTopic = GUARD_PREFIX_TOPIC + "miners/" + id + "/error";

    state = State::NotDefined;
    command = Command::Idle;
    isCommandRunning = false;
    timestamp = 0;
    commandStage = 0;
}

void Miner::runCommand() {
    switch (command) {
        case Command::PowerOn: {
            if (state == State::PoweredOff || state == State::Aborted || state == State::Unreachable) {
                timestamp = timer.getTimestamp();
                digitalWrite(pinPower, LOW);
                state = State::Starting;
            } else {
                client->publish(commandPrefixTopic + getCommandName(command), "FAILED: Wrong state!");
                return;
            }
        }
        break;

        case Command::PowerOff: {
            if (state == State::Running || state == State::Unreachable) {
                timestamp = timer.getTimestamp();
                digitalWrite(pinPower, LOW);
                state = State::Stopping;
            } else {
                client->publish(commandPrefixTopic + getCommandName(command), "FAILED: Wrong state!");
                command = Command::Idle;
                return;
            }
        }
        break;

        case Command::HardStop: {
            if (state == State::Running || state == State::Unreachable) {
                timestamp = timer.getTimestamp();
                digitalWrite(pinPower, LOW);
                state = State::HardStopping;
            } else {
                client->publish(commandPrefixTopic + getCommandName(command), "FAILED: Wrong state!");
                command = Command::Idle;
                return;
            }
        }
        break;

        case Command::Reset: {
            if (state == State::Running || state == State::Unreachable) {
                timestamp = timer.getTimestamp();
                digitalWrite(pinReset, LOW);
                state = State::Restarting;
            } else {
                client->publish(commandPrefixTopic + getCommandName(command), "FAILED: Wrong state!");
                command = Command::Idle;
                return;
            }
        }
        break;

        case Command::HardReset: {
            if (state == State::Running || state == State::Unreachable) {
                timestamp = timer.getTimestamp();
                digitalWrite(pinPower, LOW);
                state = State::HardStopping;
            } else {
                client->publish(commandPrefixTopic + getCommandName(command), "FAILED: Wrong state!");
                command = Command::Idle;
                return;
            }
        }
        case Command::LedReport: {
            /* Do nothing, just set isCommandRunning later */
        }
        break;

        case Command::NotDefined:
        case Command::Idle:
        default:
            Serial.printf(
                "Miner %s received to run undefined command %d in runCommand function!\n",
                id, static_cast<u32>(command)
            );
            Serial.flush();
            return;
        break;
    }

    isCommandRunning = true;
    commandStage = 0;
}

void Miner::watchCommandExecution() {
    switch (command)
    {
        case Command::PowerOn: {
            if (commandStage == 0) {
                if (timer.isTimeElapsed(timestamp, POWER_ON_CONTACTOR_MILISECONDS)) {
                    digitalWrite(pinPower, HIGH);
                    timestamp = timer.getTimestamp();
                    ++commandStage;
                }
            } else if (timer.isTimeElapsed(timestamp, STARTING_MILISECONDS)) {
                if (digitalRead(pinLed) == HIGH) {
                    /* Send command execution DONE */
                    client->publish(commandPrefixTopic + getCommandName(command), "DONE");
                    state = State::Running;
                } else {
                    /* Send command execution FAILED */
                    client->publish(commandPrefixTopic + getCommandName(command), "FAILED: Miner is unreachable");
                    state = State::Unreachable;
                }
                command = Command::Idle;
                isCommandRunning = false;
            }
        }
        break;

        case Command::PowerOff: {
            if (commandStage == 0) {
                if (timer.isTimeElapsed(timestamp, POWER_OFF_CONTACTOR_MILISECONDS)) {
                    digitalWrite(pinPower, HIGH);
                    timestamp = timer.getTimestamp();
                    ++commandStage;
                }
            } else if (commandStage == 1) {
                if (digitalRead(pinLed) == LOW) {
                    /* Send command execution DONE */
                    client->publish(commandPrefixTopic + getCommandName(command), "DONE");
                    state = State::PoweredOff;
                    command = Command::Idle;
                    isCommandRunning = false;

                } else if (timer.isTimeElapsed(timestamp, STOPPING_MILISECONDS)) {
                    /* Send command execution FAILED */
                    client->publish(commandPrefixTopic + getCommandName(command), "FAILED: Miner is unreachable");
                    state = State::Unreachable;
                    isCommandRunning = false;
                }
            } 
        }
        break;

        case Command::HardStop: {
            if (commandStage == 0) {
                if (timer.isTimeElapsed(timestamp, HARD_STOP_CONTACTOR_MILISECONDS)) {
                    digitalWrite(pinPower, HIGH);
                    timestamp = timer.getTimestamp();
                    ++commandStage;
                }
            } else if (commandStage == 1 && timer.isTimeElapsed(timestamp, HARD_STOPPING_MILISECONDS)) {
                if (digitalRead(pinLed) == LOW) {
                    /* Send command execution DONE */
                    client->publish(commandPrefixTopic + getCommandName(command), "DONE");
                    state = State::PoweredOff;
                } else {
                    /* Send command execution FAILED */
                    client->publish(commandPrefixTopic + getCommandName(command), "FAILED: Miner is unreachable");
                    state = State::Unreachable;
                }
                command = Command::Idle;
                isCommandRunning = false;
            }
        }
        break;

        case Command::Reset: {
            if (commandStage == 0) {
                if (timer.isTimeElapsed(timestamp, RESET_CONTACTOR_MILISECONDS)) {
                    digitalWrite(pinReset, HIGH);
                    timestamp = timer.getTimestamp();
                    ++commandStage;
                }
            } else if (commandStage == 1 && timer.isTimeElapsed(timestamp, RESETTING_MILISECONDS)) {
                if (digitalRead(pinLed) == HIGH) {
                    /* Send command execution DONE */
                    client->publish(commandPrefixTopic + getCommandName(command), "DONE");
                    state = State::Running;
                } else {
                    /* Send command execution FAILED */
                    client->publish(commandPrefixTopic + getCommandName(command), "FAILED: Miner is unreachable");
                    state = State::Aborted;
                }
                command = Command::Idle;
                isCommandRunning = false;
            }
        }
        break;

        case Command::HardReset: {
            if (commandStage == 0) {
                if (timer.isTimeElapsed(timestamp, HARD_STOP_CONTACTOR_MILISECONDS)) {
                    digitalWrite(pinPower, HIGH);
                    timestamp = timer.getTimestamp();
                    ++commandStage;
                }
            } else if (commandStage == 1) {
                if (timer.isTimeElapsed(timestamp, HARD_STOPPING_MILISECONDS)) {
                    if (digitalRead(pinLed) == LOW) {
                        digitalWrite(pinPower, LOW);
                        timestamp = timer.getTimestamp();
                        ++commandStage;
                    } else {
                        /* Send command execution FAILED */
                        client->publish(commandPrefixTopic + getCommandName(command), "FAILED: Miner is unreachable");
                        state = State::Unreachable;
                        command = Command::Idle;
                        isCommandRunning = false;
                    }
                }
            } else if (commandStage == 2) {
                if (timer.isTimeElapsed(timestamp, POWER_ON_CONTACTOR_MILISECONDS)) {
                    digitalWrite(pinPower, HIGH);
                    timestamp = timer.getTimestamp();
                    ++commandStage;
                }
            } else if (commandStage == 3) {
                if (timer.isTimeElapsed(timestamp, STARTING_MILISECONDS)) {
                    if (digitalRead(pinLed) == HIGH) {
                        /* Send command execution DONE */
                        client->publish(commandPrefixTopic + getCommandName(command), "DONE");
                        state = State::Running;
                    } else {
                        /* Send command execution FAILED */
                        client->publish(commandPrefixTopic + getCommandName(command), "FAILED: Miner is unreachable");
                        state = State::Aborted;
                    }
                    command = Command::Idle;
                    isCommandRunning = false;
                }
            }
        }
        break;
        
        case Command::LedReport: {
            if (digitalRead(pinLed) == HIGH) {
                client->publish(commandPrefixTopic + getCommandName(command), "ON");
            }
            else {
                client->publish(commandPrefixTopic + getCommandName(command), "OFF");
            }
            command = Command::Idle;
            isCommandRunning = false;
        }
        break;

        case Command::NotDefined: 
        case Command::Idle: 
        break;
        default: {
            Serial.printf(
                "Miner %s is watching not allowed command (code = %d) in watchCommandExecution function!\n",
                id, static_cast<u32>(command)
            );
            Serial.flush();
            command = Command::Idle;
            isCommandRunning = false;
        }
        break;
    }
}

void Miner::watchMinerState() {
    if (isCommandRunning)
        return;

    switch (state)
    {
        case State::PoweredOff: {
            if (digitalRead(pinLed) == HIGH) {
                state = State::Unreachable;

                /* Report problem  */
                client->publish(errorLogTopic, "[PoweredOff][ON]");
            }
        }
        break;

        case State::Running: {
            if (digitalRead(pinLed) == LOW) {
                state = State::Aborted;

                /* Report problem  */
                client->publish(errorLogTopic, "[Running][OFF]");
            }
        }
        break;

        case State::Aborted: {
            if (digitalRead(pinLed) == HIGH) {
                state = State::Unreachable;

                /* Report problem  */
                client->publish(errorLogTopic, "[Aborted][ON]");
            }
        }
        break;

        /* These states are disallowed here because they are associated with commands execution */
        case State::Starting:
        case State::Stopping:
        case State::HardStopping:
        case State::Restarting:
        case State::HardRestarting:
        case State::Unreachable: {
            Serial.printf(
                "Miner %s has state %d in watchMinerState function!\n",
                id, static_cast<u32>(state)
            );
            Serial.flush();
        }
        break;

        case State::NotDefined:
        default: {
            Serial.printf(
                "Miner %s has undefined state %d in watchMinerState function!\n",
                id, static_cast<u32>(state)
            );
            Serial.flush();
        }
        break;
    }
}

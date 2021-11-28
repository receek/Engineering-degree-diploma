#ifndef GLOBALS_H
#define GLOBALS_H

#include <Arduino.h>

#include <WiFi.h>
#include <MQTT.h>

#include "driver/timer.h"
#include "esp32-hal-cpu.h"

#include "utils.hpp"

static const u8 MAX_MINERS = 4;

static const u8 PINOUTS_SET [MAX_MINERS][3] = {
    {21, 22, 34},
    {23, 24, 35},
    {25, 26, 36},
    {32, 33, 39},
};

static const u64 POWER_ON_CONTACTOR_MILISECONDS = 1500;
static const u64 POWER_OFF_CONTACTOR_MILISECONDS = 1500;
static const u64 RESET_CONTACTOR_MILISECONDS = 1500;
static const u64 HARD_STOP_CONTACTOR_MILISECONDS = 5500;
static const u64 STARTING_MILISECONDS = 5000;
static const u64 STOPPING_MILISECONDS = 120000;
static const u64 HARD_STOPPING_MILISECONDS = 5000;
static const u64 RESETTING_MILISECONDS = 10000;

static const timer_group_t TIMER_GROUP =  TIMER_GROUP_0;
static const timer_idx_t TIMER_INDEX = TIMER_0;
static const u32 TIMER_DIVIDER = 80;

const char WIFI_SSID[] = "";
const char WIFI_PASSWORD[] = "";

const IPAddress IP_MQTT_BROKER(192, 168, 1, 106);
const int PORT_MQTT_BROKER = 1883;
const char DEV_NAME[] = "ESP32_Guard00";
const char USER[] = "broker";
const char PASSWORD[] = "broker";

const String GUARD_STARTED_TOPIC = String("guards/started");
const String GUARD_CONFIG_TOPIC = String("guards/config/") + DEV_NAME;
const String GUARD_CONFIGURED_TOPIC = String("guards/configured");
const String GUARD_PREFIX_TOPIC = String("guards/") + DEV_NAME + "/";
const String GUARD_ERROR_LOG_TOPIC = String("guards/") + DEV_NAME + "/error";

#endif
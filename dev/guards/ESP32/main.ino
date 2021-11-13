/* ESP32 board */
#include <Arduino.h>
#include <WiFi.h>
#include <MQTT.h>

#include "src/miner.hpp"
#include "src/globals.hpp"
#include "src/timer_wrapper.hpp"
#include "src/utils.hpp"


const char WIFI_SSID[] = "dom";
const char WIFI_PASSWORD[] = "0987654321";

const IPAddress IP_MQTT_BROKER(192, 168, 1, 129);
const int PORT_MQTT_BROKER = 1883;
const char DEV_NAME[] = "Mega0001";
const char USER[] = "broker";
const char PASSWORD[] = "broker";

const String GUARD_STARTED_TOPIC = String("guards/started");
const String GUARD_CONFIG_TOPIC = String("guards/config/") + DEV_NAME;
const String GUARD_CONFIGURED_TOPIC = String("guards/configured");

const String MINERS_CONTROL_TOPIC = String("guards/") + DEV_NAME + "/miners/+";

const String GUARD_POWER_RUN_TOPIC = String("guards/") + DEV_NAME + String("/power");
const String GUARD_SOFT_RESET_RUN_TOPIC = String("guards/") + DEV_NAME + String("/soft_reset");
const String GUARD_HARD_RESET_RUN_TOPIC = String("guards/") + DEV_NAME + String("/hard_reset");

const String GUARD_LED_REQUEST_TOPIC = String("guards/") + DEV_NAME + String("/LED/request");
const String GUARD_LED_STATE_TOPIC = String("guards/") + DEV_NAME + String("/LED/state");

TimerWrapper& timer = getTimerInstance();

WiFiClient network;
MQTTClient client;

Miner miners[MAX_MINERS];
u32 minersCount = -1;

bool isConfigSet = false; 
bool runPower = false;
bool runSoftReset = false;
bool runHardReset = false;
bool ledRequested = false;


const int IN_PIN = 35;
int pinPower;
int pinReset;
int pinLed;

int val = 0;

void connectMQTTClient() {
    Serial.println("MQTT Client connecting...");
    
    while (WiFi.status() != WL_CONNECTED) {
        Serial.print(".");
        delay(500);
    }

    Serial.println("WiFi connected!");

    while (!client.connect(DEV_NAME, USER, PASSWORD)) {
        delay(1000);
    }

    Serial.println("Connected!");
}


void applyConfig() {
    for (u32 i = 0; i < minersCount; ++i) {
        /* Set pinouts */
        digitalWrite(miners[i].pinPower, HIGH);
        digitalWrite(miners[i].pinReset, HIGH);
        pinMode(miners[i].pinPower, OUTPUT);
        pinMode(miners[i].pinReset, OUTPUT);
        pinMode(miners[i].pinLed, INPUT);
    
        /* Check miner state */
        delay(100);
        miners[i].state = digitalRead(miners[i].pinLed) == HIGH ?
            Running :
            PoweredOff;

    }

    /* Subscribe miner control topic */
    client.subscribe(MINERS_CONTROL_TOPIC);
}

void parseConfig(String &payload) {
    /* 
    Assume that message format is:
    "miners_count miner0_name miner0_pinset miner1_name miner1_pinset ... "
    */

    u32 from = 0;
    u32 to = 0;
    u32 length = payload.length();

    u8 pinset;
    String name;

    while (payload[to] != ' ')
      to++;
    minersCount = payload.substring(from, to).toInt();
    
    from = to = to + 1;
    
    for (u32 i = 0; i < minersCount; ++i) {
        while (payload[to] != ' ') 
            to++;
        name = payload.substring(from, to);

        from = to = to + 1;
        while (to < length && payload[to] != ' ')
            to++;
        pinset = payload.substring(from, to).toInt();

        miners[i].setConfiguration(pinset, name);
    }

    isConfigSet = true;
}

void startUpMessageReceiver(String &topic, String &payload) {
    Serial.println("incoming: " + topic + " = " + payload);

    if (topic == GUARD_CONFIG_TOPIC) {
        parseConfig(payload);
    } else {
        Serial.println("ERROR: Undefined topic arrived!");
        Serial.print("Topic: ");
        Serial.println(topic);
        Serial.print("Payload: ");
        Serial.println(payload);
    }
}

void printConfigSummary() {
    Serial.printf("Avaible miners under guard control: %d\n", minersCount);
    for (u32 i = 0; i < minersCount; ++i) {
        Serial.printf("Miner %d details:\n", i);
        Serial.printf("\tName: %s\n", miners[i].name);
        Serial.printf("\tPinouts:  power=%d, reset=%d, led=%d\n", 
            miners[i].pinPower, miners[i].pinReset, miners[i].pinLed);
        Serial.printf("\tState: %s\n", getStateName(miners[i].state));
    }
}

void setup() {
    u64 timestamp = 0;

    Serial.begin(115200);

    /* WiFi configuring */
    WiFi.begin(WIFI_SSID, WIFI_PASSWORD);

    /* MQTT connecting */
    client.begin(IP_MQTT_BROKER, PORT_MQTT_BROKER, network);
    client.onMessage(startUpMessageReceiver);
    connectMQTTClient();

    client.subscribe(GUARD_CONFIG_TOPIC);
    client.publish(GUARD_STARTED_TOPIC, DEV_NAME);
    timestamp = timer.getTimestamp();

    /* Waiting for ports configset message */
    while (!isConfigSet) {
        if (timer.isTimeElapsed(timestamp, 10000)){
            /* Republishing config request */
            client.publish(GUARD_STARTED_TOPIC, DEV_NAME);
            timestamp = timer.getTimestamp();
        }

        client.loop();
        delay(200);
    }

    client.unsubscribe(GUARD_CONFIG_TOPIC);

    /* Set pinout and subscribe all needed topics */
    applyConfig();

    /* Change message handler */

    /* Publish guard is configured */
    client.publish(GUARD_CONFIGURED_TOPIC, DEV_NAME);

    printConfigSummary();

    while (1) delay(10000);
}

void loop() {
    client.loop();
    delay(10);

    if (!client.connected()) {
        connectMQTTClient();
    }
    
    if (runPower) {
        Serial.println("Power run has been requested");

        digitalWrite(pinPower, LOW);
        delay(1000);
        digitalWrite(pinPower, HIGH);
        
        runPower = false;
    } else if (runSoftReset) {
        Serial.println("Soft reset run has been requested");
        runSoftReset = false;
    } else if (runHardReset) {
        Serial.println("Hard reset run has been requested");
        runHardReset = false;
    } else if (ledRequested) {
        Serial.println("LED state has been requested");
        ledRequested = false;
    }

    val = digitalRead(IN_PIN);
    if (val == HIGH) {
        Serial.println("In is HIGH");
    } else {
        Serial.println("In is LOW");
    }

    delay(100);
}
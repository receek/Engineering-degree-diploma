/* ESP32 board */

void setup() {
    // put your setup code here, to run once:
    Serial.begin(115200);
}

void loop() {
    // put your main code here, to run repeatedly:
    Serial.print("Hello World! i=");
    Serial.println(i++);
    delay(1000);
}
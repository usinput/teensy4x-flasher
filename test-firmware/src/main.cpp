#include <Arduino.h>

// simple blink test firmware to verify flasher works
// blinks LED in a distinctive pattern: 3 fast blinks, pause, repeat

#define LED_PIN 13

void setup() {
    pinMode(LED_PIN, OUTPUT);
}

void loop() {
    // 3 fast blinks
    for (int i = 0; i < 3; i++) {
        digitalWrite(LED_PIN, HIGH);
        delay(100);
        digitalWrite(LED_PIN, LOW);
        delay(100);
    }
    
    // long pause
    delay(1000);
}

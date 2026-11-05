plugins {
    id("com.android.application")
}

android {
    namespace = "dev.phoneuse.testbed"
    compileSdk = 37

    defaultConfig {
        applicationId = "dev.phoneuse.testbed"
        minSdk = 26
        targetSdk = 37
        versionCode = 1
        versionName = "1"
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
}

dependencies {
    implementation("androidx.recyclerview:recyclerview:1.4.0")
}

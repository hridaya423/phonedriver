package dev.phonedriver.testbed

import android.app.Activity
import android.view.View
import androidx.compose.foundation.layout.Row
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TimePicker
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.material3.rememberTimePickerState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.ComposeView
import java.time.Instant
import java.time.ZoneOffset
import java.util.Locale

@OptIn(ExperimentalMaterial3Api::class)
object ComposePickers {
    @JvmStatic
    fun create(activity: Activity): View {
        val view = ComposeView(activity)
        view.setContent {
            var date by remember { mutableStateOf(false) }
            var time by remember { mutableStateOf(false) }
            Row {
                Button(onClick = { date = true }) { Text("Compose date") }
                Button(onClick = { time = true }) { Text("Compose time") }
            }
            if (date) {
                val state = rememberDatePickerState(initialSelectedDateMillis = 1768435200000L)
                DatePickerDialog(
                    onDismissRequest = { date = false },
                    confirmButton = {
                        TextButton(onClick = {
                            date = false
                            state.selectedDateMillis?.let {
                                val day = Instant.ofEpochMilli(it).atZone(ZoneOffset.UTC).toLocalDate()
                                Pickers.picked("compose_date", day.toString())
                            }
                        }) { Text("OK") }
                    },
                    dismissButton = { TextButton(onClick = { date = false }) { Text("Cancel") } },
                ) { DatePicker(state = state) }
            }
            if (time) {
                val state = rememberTimePickerState(initialHour = 9, initialMinute = 5, is24Hour = false)
                AlertDialog(
                    onDismissRequest = { time = false },
                    confirmButton = {
                        TextButton(onClick = {
                            time = false
                            Pickers.picked(
                                "compose_time",
                                String.format(Locale.ROOT, "%02d:%02d", state.hour, state.minute),
                            )
                        }) { Text("OK") }
                    },
                    dismissButton = { TextButton(onClick = { time = false }) { Text("Cancel") } },
                    text = { TimePicker(state = state) },
                )
            }
        }
        return view
    }
}

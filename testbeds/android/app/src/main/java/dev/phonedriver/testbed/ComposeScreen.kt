package dev.phonedriver.testbed

import android.app.Activity
import android.view.View
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.material3.Button
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.ComposeView
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.testTagsAsResourceId
import androidx.compose.ui.unit.dp
import org.json.JSONObject

@OptIn(androidx.compose.ui.ExperimentalComposeUiApi::class)
object ComposeScreen {
    @JvmStatic
    fun create(activity: Activity): View {
        val view = ComposeView(activity)
        view.setContent {
            var text by remember { mutableStateOf("") }
            Column(Modifier.fillMaxSize().semantics { testTagsAsResourceId = true }) {
                Text("Compose title", Modifier.semantics { heading() })
                Button(
                    onClick = { Events.emit(Events.put(Events.event("compose_click"), "id", "compose_button")) },
                    modifier = Modifier.testTag("compose_button"),
                ) { Text("Compose button") }
                Row {
                    listOf("compose_switch", "compose_switch_2", "compose_switch_3").forEach { tag ->
                        var checked by remember { mutableStateOf(false) }
                        Switch(
                            checked = checked,
                            onCheckedChange = {
                                checked = it
                                val o = Events.event("compose_toggle")
                                Events.put(o, "id", tag)
                                Events.put(o, "on", it)
                                Events.emit(o)
                            },
                            modifier = Modifier.testTag(tag),
                        )
                    }
                }
                BasicTextField(
                    value = text,
                    onValueChange = {
                        text = it
                        val o = Events.event("compose_text")
                        Events.put(o, "value", it)
                        Events.emit(o)
                    },
                    modifier = Modifier.fillMaxWidth().padding(8.dp).testTag("compose_field"),
                )
                LazyColumn(Modifier.fillMaxSize().testTag("compose_list")) {
                    items((0 until 1000).toList()) { n ->
                        Text(
                            "Item $n",
                            Modifier.fillMaxWidth().padding(12.dp).clickable {
                                Events.emit(Events.put(Events.event("compose_item"), "n", n))
                            },
                        )
                    }
                }
            }
        }
        return view
    }
}

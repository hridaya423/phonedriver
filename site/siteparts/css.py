CSS = r"""
@font-face{font-family:"Inter Display";src:url(/assets/fonts/inter-display.woff2) format("woff2");font-weight:400;font-style:normal;font-display:swap}
@font-face{font-family:"Commit Mono";src:url(/assets/fonts/commit-mono.woff2) format("woff2");font-weight:400;font-style:normal;font-display:optional}
@font-face{font-family:"Inter Fallback";src:local("Arial");size-adjust:@@ISA@@%;ascent-override:@@IAS@@%;descent-override:@@IDE@@%;line-gap-override:0%}
@font-face{font-family:"Mono Fallback";src:local("Menlo"),local("Consolas"),local("Courier New");size-adjust:@@MSA@@%;ascent-override:@@MAS@@%;descent-override:@@MDE@@%;line-gap-override:0%}
:root{--page:#fff;--panel:#fafaf7;--well:#f4f3ee;--sel:#fff4bf;--t1:#161512;--t2:#55524a;--t3:#6b675e;--line:#e2dfd5;--accent:#ffd400;--atext:#7a5d00;--ok:#2c6a3a;--fail:#b3261e;--bar:#9a937c;--bar2:#5b4a1a;--sans:"Inter Display","Inter Fallback",Arial,sans-serif;--mono:"Commit Mono","Mono Fallback",Menlo,Consolas,monospace}
@media (prefers-color-scheme:dark){:root:not([data-theme=light]){--page:#141412;--panel:#1c1b19;--well:#22211e;--sel:#3a3107;--t1:#f5f3ee;--t2:#c4c0b6;--t3:#a19c90;--line:#2c2a25;--atext:#ffd400;--ok:#8cc79a;--fail:#ff6b57;--bar:#8f8a76;--bar2:#ebdc9c}}
:root[data-theme=dark]{--page:#141412;--panel:#1c1b19;--well:#22211e;--sel:#3a3107;--t1:#f5f3ee;--t2:#c4c0b6;--t3:#a19c90;--line:#2c2a25;--atext:#ffd400;--ok:#8cc79a;--fail:#ff6b57;--bar:#8f8a76;--bar2:#ebdc9c}
*,*::before,*::after{box-sizing:border-box}
html{-webkit-text-size-adjust:100%;scroll-behavior:smooth;scroll-padding-top:80px}
body{margin:0;background:var(--page);color:var(--t1);font:400 18px/1.6 var(--sans);font-synthesis:none;font-kerning:normal;text-rendering:optimizeLegibility;-webkit-font-smoothing:antialiased}
b,strong,h1,h2,h3,h4,th{font-weight:400}
img,svg,video{max-width:100%;height:auto;display:block}
a{color:inherit;text-decoration:underline;text-decoration-color:var(--accent);text-decoration-thickness:2px;text-underline-offset:3px}
a:hover{background:var(--sel)}
a:focus-visible,pre:focus-visible{outline:2px solid var(--t1);outline-offset:2px}
.skip{position:absolute;left:-999px;top:8px;background:var(--accent);color:#161512;padding:8px 12px;z-index:10}
.skip:focus{left:8px}
.wrap{max-width:1160px;margin:0 auto;padding:0 20px}
.col{max-width:760px}.col.wide{max-width:none}
.lab{font:400 12px/1.4 var(--mono);letter-spacing:.08em;text-transform:uppercase;color:var(--t3);margin:0 0 12px}
.small{font-size:15px;color:var(--t2);line-height:1.5}
code,pre,kbd{font-family:var(--mono);font-size:.84em}
code{background:var(--well);padding:.08em .35em}
pre{background:var(--panel);border:1px solid var(--line);margin:20px 0;padding:16px 18px;overflow-x:auto;font-size:14px;line-height:1.65;white-space:pre}
pre code{background:none;padding:0;font-size:inherit}
.cmd{color:var(--t1)}
.add{color:var(--ok)}.del{color:var(--fail)}.chg{color:var(--atext);background:var(--sel)}
.top{border-bottom:1px solid var(--line);position:sticky;top:0;background:var(--page);z-index:5}
.bar{display:flex;align-items:center;justify-content:space-between;gap:16px;min-height:64px;flex-wrap:wrap;padding-top:8px;padding-bottom:8px}
.brand{display:flex;align-items:center;gap:10px;font-size:22px;text-decoration:none;color:var(--t1)}
.brand:hover,.nav a:hover{background:none}
.mark{color:var(--t1);flex:none}
.nav{display:flex;gap:6px 22px;flex-wrap:wrap}
.nav a{font:400 13px/1 var(--mono);letter-spacing:.06em;text-transform:uppercase;text-decoration:none;color:var(--t2);padding:10px 0;border-bottom:2px solid transparent}
.nav a:hover{color:var(--t1);border-bottom-color:var(--line)}
.nav a[aria-current]{color:var(--t1);border-bottom-color:var(--accent)}
.crumbs{max-width:1160px;margin:0 auto;padding:20px 20px 0;font:400 12px/1.4 var(--mono);letter-spacing:.06em;text-transform:uppercase;color:var(--t3)}
.crumbs a{color:var(--t3);text-decoration:none}.crumbs a:hover{color:var(--t1);background:none}
h1{font-size:clamp(38px,6.4vw,76px);line-height:1.02;letter-spacing:-.022em;margin:0 0 24px}
h2{font-size:clamp(28px,3.6vw,42px);line-height:1.1;letter-spacing:-.018em;margin:0}
h3{font-size:22px;line-height:1.25;margin:32px 0 8px;letter-spacing:-.01em}
.hl{background:linear-gradient(var(--accent),var(--accent)) 0 92%/100% 8px no-repeat;padding-bottom:2px;-webkit-box-decoration-break:clone;box-decoration-break:clone}
.hero>*,.two>*,.installs>*,.docs>*,.cards>*,.fgrid>*,.faq>*{min-width:0}
.hero{display:grid;grid-template-columns:minmax(0,1fr);gap:40px;padding-top:56px;padding-bottom:56px}
.lede{font-size:clamp(19px,2.2vw,23px);line-height:1.45;color:var(--t2);max-width:36em;margin:0 0 20px}
.def{color:var(--t1)}
.meta{font:400 13px/1.5 var(--mono);letter-spacing:.04em;color:var(--t3);margin:0 0 28px}
.cta{display:flex;gap:12px;flex-wrap:wrap;margin:0}
.btn{display:inline-block;background:var(--accent);color:#161512;text-decoration:none;padding:14px 22px;font:400 14px/1 var(--mono);letter-spacing:.06em;text-transform:uppercase;border:1px solid var(--accent)}
.btn:hover{background:#ffe04d;color:#161512}
.btn.ghost{background:none;color:var(--t1);border-color:var(--t1)}
.btn.ghost:hover{background:var(--sel)}
.scr{margin:0}.scr pre{margin:0 0 12px;background:var(--panel);white-space:pre-wrap;overflow-wrap:anywhere}
.scr pre code .chg{display:inline}
.sec{border-top:1px solid var(--line);padding:48px 0 56px}
.sh{display:flex;align-items:baseline;gap:16px;margin-bottom:28px;flex-wrap:wrap}
.sh .lab{margin:0}
.sec p{max-width:44em}
.two{display:grid;gap:28px;grid-template-columns:minmax(0,1fr);align-items:start}
.two pre{margin-top:0;white-space:pre-wrap;overflow-wrap:anywhere}
.installs{display:grid;gap:20px;grid-template-columns:minmax(0,1fr)}
.ibox pre{margin:0}
.note{grid-column:1/-1;margin:0}
.chart{margin:0 0 20px;border:1px solid var(--line)}
.chart img{width:100%}
.demo{margin:0}
.demo video{width:100%;border:1px solid var(--line)}
.demo figcaption{margin-top:12px;max-width:44em}
.slot-demo{border:1px dashed var(--t3);aspect-ratio:960/400;background:var(--panel);display:flex;flex-direction:column;justify-content:center;padding:clamp(20px,5vw,56px);max-width:960px}
.slot-demo .big{font-size:clamp(22px,3.6vw,38px);line-height:1.15;letter-spacing:-.015em;margin:0 0 12px;max-width:18em}
.tw{overflow-x:auto;margin:20px 0;border:1px solid var(--line);background:var(--page)}
table{border-collapse:collapse;width:100%;font-size:15px;line-height:1.45;min-width:560px}
th,td{padding:11px 14px;border-bottom:1px solid var(--line);text-align:left;vertical-align:top}
thead th{font:400 12px/1.3 var(--mono);letter-spacing:.06em;text-transform:uppercase;color:var(--t3);background:var(--panel);position:sticky;top:0}
tbody th{color:var(--t1)}
tr:last-child>*{border-bottom:0}
td{font-variant-numeric:tabular-nums}
td code{overflow-wrap:anywhere}
tbody th{min-width:9.5em}
.slot{font-family:var(--mono);background:var(--sel);color:var(--atext);white-space:nowrap}
.form{border:1px solid var(--t1);padding:18px 20px;margin:0;background:var(--panel)}
.form p{margin:6px 0}.form .ft{font-size:20px;margin-bottom:12px}
.form b{font-weight:400;background:var(--sel);padding:0 .2em}
.choices{display:flex;gap:8px;flex-wrap:wrap;margin:16px 0!important}
.choices span{border:1px solid var(--t1);padding:8px 14px;font:400 13px/1 var(--mono);text-transform:uppercase;letter-spacing:.05em}
.choices span:first-child{background:var(--accent);border-color:var(--accent);color:#161512}
.faq{display:grid;gap:0 64px;grid-template-columns:minmax(0,1fr)}
.qa{padding:22px 0;border-top:1px solid var(--line)}
.qa:first-child{border-top:0;padding-top:0}
@media (min-width:1000px){.faq{grid-template-columns:1fr 1fr}.qa:nth-child(2){border-top:0;padding-top:0}.qa{break-inside:avoid}}
.qa h3{margin:0 0 8px;font-size:23px;line-height:1.25}
.qa p{margin:0;color:var(--t2)}
.pagehead{padding:56px 0 24px}
.pagehead h1{font-size:clamp(34px,5vw,60px);max-width:16em}
.cards{display:grid;gap:0;grid-template-columns:1fr;border-top:1px solid var(--line);margin-bottom:64px}
.card{display:block;padding:24px 4px;border-bottom:1px solid var(--line);text-decoration:none;color:var(--t1)}
.card:hover{background:var(--sel)}
.card h2{font-size:24px;margin:0 0 8px;letter-spacing:-.01em;line-height:1.2}
.card p{margin:0;color:var(--t2);font-size:16px;max-width:46em}
.card .lab{margin-bottom:8px}
.prose{padding:48px 0 72px}
.prose>.lab{margin-bottom:16px}
.prose h1{font-size:clamp(32px,4.6vw,52px);line-height:1.08}
.prose h2{font-size:clamp(26px,3vw,34px);margin:56px 0 16px;line-height:1.15}
.prose h3{margin:36px 0 8px}
.prose p,.prose li{max-width:40em}
.prose ul,.prose ol{padding-left:1.3em;margin:16px 0}
.prose li{margin:6px 0}
.prose li::marker{color:var(--t3)}
.prose blockquote{margin:24px 0;padding:4px 0 4px 20px;border-left:4px solid var(--accent);color:var(--t2)}
.prose blockquote p{margin:0}
.prose .tw{max-width:none}
.byline{margin:-4px 0 20px}
.toc{border:1px solid var(--line);padding:16px 20px;margin:24px 0 8px;background:var(--panel)}
.toc ol{margin:0;padding-left:1.2em}.toc .lab{margin-bottom:8px}.toc li{margin:4px 0;font-size:16px}
.toc2{font-size:16px}
.post h2{margin-top:48px}
.docs{display:grid;grid-template-columns:minmax(0,1fr);gap:0 56px}
.side{padding:28px 0 0}
.side nav{display:flex;flex-wrap:wrap;gap:4px 18px}
.side nav a{font-size:15px;color:var(--t2);text-decoration:none;padding:6px 0;border-bottom:2px solid transparent}
.side nav a:hover{background:none;color:var(--t1);border-bottom-color:var(--line)}
.side nav a[aria-current]{color:var(--t1);border-bottom-color:var(--accent)}
.pnav{display:grid;grid-template-columns:1fr 1fr;gap:16px;margin-top:56px;border-top:1px solid var(--line);padding-top:20px}
.pn{text-decoration:none;display:block;padding:8px 0}.pn.next{text-align:right;grid-column:2}
.pn .lab{display:block;margin-bottom:4px}
.bblock{padding:20px 0;border-top:1px solid var(--line)}
.bblock h3{margin:0 0 12px;font-size:20px}
.brow{display:grid;grid-template-columns:104px 1fr;gap:4px 12px;align-items:center;margin:6px 0}
.bn{font:400 12px/1 var(--mono);letter-spacing:.04em;color:var(--t3)}
.bt{position:relative;height:16px;background:var(--faint,transparent);display:block}
.bt.na{height:auto;font:400 13px/1 var(--mono);color:var(--t3)}
.fill{display:block;height:16px}
.fill.pd{background:var(--accent)}.fill.adb{background:var(--bar)}.fill.ph{background:var(--bar2)}
.tick{position:absolute;top:-2px;bottom:-2px;width:2px;background:var(--t1)}
.bv{grid-column:2;font:400 13px/1.2 var(--mono);color:var(--t1);font-variant-numeric:tabular-nums}
.bv small{color:var(--t3);font-size:12px}
.bblock .small{margin:8px 0 0}
.foot{border-top:1px solid var(--line);padding:48px 0 64px;background:var(--panel)}
.fgrid{display:grid;gap:32px;grid-template-columns:1fr}
.foot nav{display:flex;flex-direction:column;gap:8px}
.foot nav a{text-decoration:none;color:var(--t2);font-size:16px;width:max-content}
.foot nav a:hover{color:var(--t1)}
.fbrand{display:flex;gap:10px;align-items:center;font-size:22px;margin:0 0 12px}
.legal{margin-top:40px;max-width:60em}
@media (max-width:759px){
body{font-size:17px}
.nav{gap:2px 16px}.nav a{font-size:12px;letter-spacing:.04em}
.ibox pre{white-space:pre-wrap;overflow-wrap:anywhere}
.stack{border:0;overflow:visible}
.stack table{min-width:0;display:block}
.stack thead{display:none}
.stack tbody{display:block}
.stack tr{display:block;border:1px solid var(--line);margin:0 0 14px;background:var(--page)}
.stack th,.stack td{display:block;border:0;padding:8px 14px}
.stack tbody th{background:var(--panel);min-width:0;border-bottom:1px solid var(--line)}
.stack td{display:grid;grid-template-columns:7.2em minmax(0,1fr);gap:10px}
.stack td::before{content:attr(data-label);font:400 11px/1.6 var(--mono);letter-spacing:.06em;text-transform:uppercase;color:var(--t3)}
}
@media (min-width:760px){
.wrap{padding:0 32px}
.crumbs{padding:24px 32px 0}
.hero{grid-template-columns:minmax(0,7fr) minmax(0,5fr);gap:56px;padding-top:88px;padding-bottom:80px;align-items:start}
.hero .scr{margin-top:12px}
.two{grid-template-columns:1fr 1fr;gap:48px}
.installs{grid-template-columns:1fr 1fr}
.cards{grid-template-columns:1fr 1fr}
.cards.one{grid-template-columns:1fr}
.card{padding:28px 24px 28px 0;border-right:0}
.cards:not(.one) .card:nth-child(odd){padding-right:32px}
.cards:not(.one) .card:nth-child(even){padding-left:32px;border-left:1px solid var(--line)}
.fgrid{grid-template-columns:2fr 1fr 1fr}
.brow{grid-template-columns:120px 1fr 190px}
.bv{grid-column:auto}
}
@media (min-width:1000px){
.docs{grid-template-columns:220px minmax(0,1fr)}
.side{padding:56px 0}
.side nav{flex-direction:column;position:sticky;top:96px;gap:2px}
.side nav a{border-bottom:0;border-left:2px solid transparent;padding:6px 0 6px 12px;margin-left:-12px}
.side nav a:hover{border-left-color:var(--line)}
.side nav a[aria-current]{border-left-color:var(--accent)}
}
@media (prefers-reduced-motion:reduce){html{scroll-behavior:auto}}
@media print{.top,.foot,.side{display:none}}
"""

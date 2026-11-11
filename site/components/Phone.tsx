import { preload } from "react-dom";

export function Phone() {
  preload("/assets/run-first.jpg", { as: "image", fetchPriority: "high" });
  return (
    <div className="ip" aria-hidden="true">
      <div className="ip-screen">
        <video className="ip-video" src="/assets/run.mp4" poster="/assets/run-first.jpg" width={540} height={1212} muted playsInline preload="auto" disablePictureInPicture />
        <span className="ip-held">
          <b>held for approval</b>
        </span>
        <span className="ip-touch" />
      </div>
      <noscript>
        <style>{".ip-screen::before{content:'';position:absolute;inset:0;background:url(/assets/run-poster.jpg) 50%/cover}"}</style>
      </noscript>
    </div>
  );
}

import type { EditorUiModule } from "@the-editor/editor/ui";
import "./doom.css";

const doomToolTypeId = "doom-lab.tool";
const doomUrl = "https://js-dos.com/games/doom.exe.html";

const doomLabUi: EditorUiModule = (host) => {
  host.tools.register({
    typeId: doomToolTypeId,
    render({ commands }) {
      return <DoomSurface close={() => commands.dispatch({ id: "doom_lab.close" })} />;
    },
  });
};

export default doomLabUi;

function DoomSurface({ close }: { close: () => Promise<unknown> }) {
  return (
    <section className="doomLabOverlay">
      <div className="doomLabChrome">
        <div>
          <div className="doomLabTitle">Doom</div>
          <div className="doomLabMeta">shareware build in an iframe</div>
        </div>
        <div className="doomLabActions">
          <a className="doomLabButton" href={doomUrl} target="_blank" rel="noreferrer">open in browser</a>
          <button className="doomLabButton" type="button" onClick={() => { void close(); }}>close</button>
        </div>
      </div>

      <div className="doomLabFrameWrap">
        <iframe
          className="doomLabFrame"
          src={doomUrl}
          allow="autoplay; fullscreen; gamepad"
          referrerPolicy="no-referrer"
          title="Doom"
        />
      </div>

      <div className="doomLabHint">click inside the frame to give Doom keyboard focus</div>
    </section>
  );
}

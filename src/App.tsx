import { useState } from "react";
import reactLogo from "./assets/react.svg";
import { invoke } from "@tauri-apps/api/core";

function App() {
  const [greetMsg, setGreetMsg] = useState("");
  const [name, setName] = useState("");

  async function greet() {
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    setGreetMsg(await invoke("greet", { name }));
  }

  return (
    <main className="container bg-slate-50 text-slate-900">
      <h1 className="text-4xl font-bold tracking-tight text-slate-900 sm:text-5xl">
        Welcome to Tauri + React
      </h1>

      <div className="row mt-8">
        <a href="https://tauri.app" target="_blank" rel="noreferrer">
          <img src="/tauri.svg" className="logo tauri" alt="Tauri logo" />
        </a>
        <a href="https://react.dev" target="_blank" rel="noreferrer">
          <img src={reactLogo} className="logo react" alt="React logo" />
        </a>
      </div>
      <p className="mt-6 text-slate-600">
        Click on the Tauri and React logos to learn more.
      </p>

      <form
        className="form-row"
        onSubmit={(e) => {
          e.preventDefault();
          greet();
        }}
      >
        <input
          id="greet-input"
          className="border-gray-300 bg-white text-slate-900"
          onChange={(e) => setName(e.currentTarget.value)}
          placeholder="Enter a name..."
        />
        <button type="submit" className="bg-slate-900 text-white hover:bg-slate-800">
          Greet
        </button>
      </form>
      <p className="mt-4 text-slate-700">{greetMsg}</p>
    </main>
  );
}

export default App;

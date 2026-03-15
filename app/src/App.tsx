import { Routes, Route } from "react-router-dom";
import { Layout } from "./components/Layout.tsx";
import { BuilderPage } from "./components/builder/BuilderPage.tsx";

export function App() {
  return (
    <Layout>
      <Routes>
        <Route path="/" element={<BuilderPage />} />
      </Routes>
    </Layout>
  );
}

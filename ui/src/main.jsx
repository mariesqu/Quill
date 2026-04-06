import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./styles/globals.css";

class ErrorBoundary extends React.Component {
  constructor(props) {
    super(props);
    this.state = { hasError: false };
  }
  static getDerivedStateFromError() {
    return { hasError: true };
  }
  componentDidCatch(error, info) {
    console.error("Quill crashed:", error, info);
  }
  render() {
    if (this.state.hasError) {
      return (
        <div style={{ padding: 32, textAlign: "center", color: "#ccc" }}>
          <h2>Something went wrong</h2>
          <p>Please restart Quill. If the issue persists, check the logs.</p>
        </div>
      );
    }
    return this.props.children;
  }
}

ReactDOM.createRoot(document.getElementById("root")).render(
  <React.StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </React.StrictMode>
);

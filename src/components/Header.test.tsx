import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { I18nProvider } from "../i18n/context";
import Header from "./Header";

function renderWithProviders(ui: React.ReactElement) {
  return render(<I18nProvider>{ui}</I18nProvider>);
}

describe("Header", () => {
  it("renders without crashing when status is null (white screen regression)", () => {
    // This would have thrown before the null-guard fix was applied
    renderWithProviders(
      <Header
        status={null}
        loading={false}
        onRefresh={() => {}}
        onSettings={() => {}}
      />
    );

    // Should show the title even without status
    expect(screen.getByText("Open Proxy AI")).toBeInTheDocument();
  });

  it("renders loading state without crashing", () => {
    renderWithProviders(
      <Header
        status={null}
        loading={true}
        onRefresh={() => {}}
        onSettings={() => {}}
      />
    );

    expect(screen.getByText("Open Proxy AI")).toBeInTheDocument();
  });

  it("shows port number when status is available", () => {
    renderWithProviders(
      <Header
        status={{ running: true, port: 6446, model_count: 5, keys: [], custom_models: [] }}
        loading={false}
        onRefresh={() => {}}
        onSettings={() => {}}
      />
    );

    // The URL span should render with the port
    expect(screen.getByText("http://localhost:6446")).toBeInTheDocument();
  });

  it("does not show port URL when status is null", () => {
    renderWithProviders(
      <Header
        status={null}
        loading={false}
        onRefresh={() => {}}
        onSettings={() => {}}
      />
    );

    // The URL span should NOT be present
    expect(screen.queryByText(/http:\/\/localhost/)).not.toBeInTheDocument();
  });
});

Rails.application.routes.draw do
  root "notes#index"
  get "/notes/:id", to: "notes#show"
  namespace :admin, path: "staff", module: "backoffice" do
    get "notes", to: "notes#index"
    post "global", to: "/notes#create"
  end
  scope "/api", module: "v1" do
    get "notes", to: "notes#index"
  end
  resources :notes, only: [:index, :show, :update]
  resource :profile, controller: "profiles", only: :show
end

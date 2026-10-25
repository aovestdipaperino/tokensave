Rails.application.routes.draw do
  resources :users
  resource :profile
  resources :people do
    resources :posts, only: :index
  end
  resources :photos, only: [] do
    member do
      get 'preview'
      post :rotate
    end
    collection do
      get :search
    end
    get :draft, on: :new
    get 'pre-view', on: :member
    get 'thumbnail'
  end
  resource :account, only: [] do
    resources :keys, only: :index
    get 'settings'
  end
  get 'legacy' => 'pages#legacy'
  post 'admin/reports' => 'admin/reports#create'
  match 'search', to: 'search#index', via: [:get, :post]
  get 'photos/archive'
  namespace :admin do
    get 'stats/daily'
    resources :users, only: :show, module: 'staff'
  end
  resources :items, controller: 'goods', path: 'stuff', only: :show, param: :slug
  resources :users, only: [] do
    scope module: :users do
      resources :notes, only: :index
    end
    namespace :x do
      get 'a', to: 'b#c'
    end
  end
  controller :sessions do
    get 'login', action: :new
  end
  constraints subdomain: 'api' do
    get 'status', to: 'health#show'
  end
  defaults format: :json do
    get 'feed', to: 'feed#index'
  end
  namespace :staff do
    scope path: nil do
      get 'audit', to: 'logs#index'
    end
  end
  get 'old', to: redirect('/new')
  mount ->(env) { [200, {}, []] }, at: '/rack'
  root 'home#index'
end

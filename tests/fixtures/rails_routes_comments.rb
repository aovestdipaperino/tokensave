Rails.application.routes.draw do
  get '/notes', # path comment
      to: 'notes#show' # target comment
  scope '/staff', # scope comment
        module: 'admin' do
    get '/notes', { # hash comment
      to: 'notes#show', # option comment
    }
  end
  resources :notes, only: [
    :index, # included action
    :show,
    :update,
  ], except: [
    :index, # excluded action
  ]
end

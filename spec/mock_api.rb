# frozen_string_literal: true

require 'http_decoy'

MOCK_API = File.read('assets/mock_api.json')

HttpDecoy.configure do |c|
  c.auto_intercept = false # opt out of WebMock auto-interception
end

MockApi = HttpDecoy.define(:mockapi) do
  get '/docs/api.json' do
    respond 200, text: MOCK_API
  end

  get '/api/v1/hello/' do
    respond 200, json: { msg: 'Hello, World!' }
  end
end

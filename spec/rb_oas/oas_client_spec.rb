# frozen_string_literal: true

require 'http_decoy/rspec'
require 'rb_oas'

RSpec.describe RbOas::OasClient do
  describe 'new Client' do
    let :server do
      fake_server(:mockapi)
    end

    let :port do
      server.port
    end

    let :client do
      described_class.new "http://127.0.0.1:#{port}/", 'docs/api.json'
    end

    it 'errors on unknown operation_id' do
      expect { client.call 'heyhey' }.to raise_error RuntimeError
    end

    it 'calls hello' do
      result = client.call 'hello'
      expect(result).to eq({ 'msg' => 'Hello, World!' })
    end
  end
end

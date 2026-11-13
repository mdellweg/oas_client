# frozen_string_literal: true

require_relative 'rb_oas/version'

begin
  require 'rb_oas/rb_oas'
rescue LoadError => e
  raise unless e.path == 'rb_oas/rb_oas'

  # A local build of a git-sourced gem may leave the extension in its crate directory.
  require_relative '../crates/rb_oas/rb_oas'
end

module RbOas
  class Error < StandardError; end
  # Your code goes here...
end

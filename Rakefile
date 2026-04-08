# frozen_string_literal: true

require 'bundler/gem_tasks'
require 'rubocop/rake_task'

RuboCop::RakeTask.new

require 'rb_sys/extensiontask'

desc 'Compile ruby extension'
task build: :compile

GEMSPEC = Gem::Specification.load('rb_oas.gemspec')

RbSys::ExtensionTask.new('rb_oas', GEMSPEC) do |ext|
  ext.lib_dir = 'lib/rb_oas'
end

task default: %i[compile rubocop]
